use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use surrealdb::engine::any::Any;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;
use tracing::{debug, info, warn};
use uuid::Uuid;

// Custom serde module for serializing UUID as string
mod uuid_as_string {
    use super::*;

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
        let s = String::deserialize(deserializer)?;
        Uuid::parse_str(&s).map_err(serde::de::Error::custom)
    }
}

// Custom serde module for Option<UUID> as Option<String>
mod option_uuid_as_string {
    use super::*;

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
        Option::<String>::deserialize(deserializer)?
            .map(|s| Uuid::parse_str(&s).map_err(serde::de::Error::custom))
            .transpose()
    }
}

// Custom serde module for DateTime as RFC3339 string compatible with SurrealDB
mod datetime_as_rfc3339 {
    use super::*;

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as RFC3339 string for SurrealDB compatibility
        serializer.serialize_str(&dt.to_rfc3339())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

// Custom serde module for Option<DateTime>
mod option_datetime_as_rfc3339 {
    use super::*;

    pub fn serialize<S>(opt_dt: &Option<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match opt_dt {
            Some(dt) => serializer.serialize_some(&dt.to_rfc3339()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(serde::de::Error::custom)
            })
            .transpose()
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
    #[serde(with = "option_uuid_as_string")]
    pub user_id: Option<Uuid>,
    #[serde(with = "datetime_as_rfc3339")]
    pub upload_timestamp: DateTime<Utc>,
    pub status: String, // 'uploaded', 'queued', 'processing', 'completed', 'failed'
    pub content: String,
    pub page_count: Option<i32>,
    pub processing_progress: Option<f32>, // 0.0 to 1.0
    pub error_message: Option<String>,
    pub retry_count: i32,
    #[serde(with = "option_datetime_as_rfc3339")]
    pub processing_started_at: Option<DateTime<Utc>>,
    #[serde(with = "option_datetime_as_rfc3339")]
    pub processing_completed_at: Option<DateTime<Utc>>,
    #[serde(with = "datetime_as_rfc3339")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "datetime_as_rfc3339")]
    pub updated_at: DateTime<Utc>,
}

/// Document embedding chunk with vector data
/// Note: UUID fields are serialized as strings for SurrealDB compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEmbedding {
    #[serde(with = "uuid_as_string")]
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
    #[serde(with = "datetime_as_rfc3339")]
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

    // ====================
    // Document CRUD Operations
    // ====================

    /// Create a new document
    pub async fn create_document(&self, document: Document) -> Result<Document> {
        debug!("Creating document: {}", document.document_id);

        let created: Option<Document> = self
            .db
            .create(("documents", document.document_id.to_string()))
            .content(document)
            .await
            .map_err(|e| {
                tracing::error!("SurrealDB create error: {:?}", e);
                anyhow::anyhow!("Failed to create document: {}", e)
            })?;

        created.ok_or_else(|| anyhow::anyhow!("Failed to create document: no result returned"))
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

        let mut updated_doc = document.clone();
        updated_doc.updated_at = Utc::now();

        let updated: Option<Document> = self
            .db
            .update(("documents", document.document_id.to_string()))
            .content(updated_doc)
            .await
            .context("Failed to update document")?;

        updated.context("Failed to update document: no result returned")
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

        let documents: Vec<Document> = result.take(0).context("Failed to parse documents")?;

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
        let mut query = String::from("UPDATE $document_id SET status = $status, updated_at = $now");

        // Add processing timestamps based on status
        if status == "processing" {
            query.push_str(", processing_started_at = $now");
        } else if status == "completed" || status == "failed" {
            query.push_str(", processing_completed_at = $now");
        }

        // Add optional fields
        if let Some(prog) = progress {
            query.push_str(&format!(", processing_progress = {}", prog));
        }
        if error_message.is_some() {
            query.push_str(", error_message = $error_message");
        }

        let mut db_query = self
            .db
            .query(query)
            .bind(("document_id", format!("documents:{}", document_id)))
            .bind(("status", status_owned))
            .bind(("now", now));

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

        let query =
            "UPDATE $document_id SET retry_count += 1, updated_at = time::now() RETURN retry_count";

        let mut result = self
            .db
            .query(query)
            .bind(("document_id", format!("documents:{}", document_id)))
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
        debug!(
            "Attempting to claim document {} for processing",
            document_id
        );

        // Atomic update: only update if status is 'uploaded' or 'queued'
        let query = r#"
            UPDATE $document_id SET
                status = 'processing',
                processing_started_at = time::now(),
                updated_at = time::now()
            WHERE status IN ['uploaded', 'queued']
        "#;

        let mut result = self
            .db
            .query(query)
            .bind(("document_id", format!("documents:{}", document_id)))
            .await
            .context("Failed to claim document for processing")?;

        let updated: Vec<Document> = result.take(0).unwrap_or_default();
        let claimed = !updated.is_empty();

        if claimed {
            info!(
                "Successfully claimed document {} for processing",
                document_id
            );
        } else {
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

        let created: Option<DocumentEmbedding> = self
            .db
            .create(("document_embeddings", embedding.embedding_id.to_string()))
            .content(embedding)
            .await
            .context("Failed to create embedding")?;

        created.context("Failed to create embedding: no result returned")
    }

    /// Create multiple embeddings in batch
    pub async fn create_embeddings_batch(
        &self,
        embeddings: Vec<DocumentEmbedding>,
    ) -> Result<Vec<DocumentEmbedding>> {
        info!("Creating {} embeddings in batch", embeddings.len());

        let mut created_embeddings = Vec::new();

        for embedding in embeddings {
            let created = self.create_embedding(embedding).await?;
            created_embeddings.push(created);
        }

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
        let query = if company_id.is_some() {
            format!(
                "SELECT embedding_id, document_id, company_id, deal_id, chunk_text,
                 chunk_index, chunk_position, page_number,
                 vector::similarity::cosine(embedding, $query_vector) AS score
                 FROM document_embeddings
                 WHERE company_id = $company_id
                 ORDER BY score DESC
                 LIMIT {}",
                limit
            )
        } else {
            format!(
                "SELECT embedding_id, document_id, company_id, deal_id, chunk_text,
                 chunk_index, chunk_position, page_number,
                 vector::similarity::cosine(embedding, $query_vector) AS score
                 FROM document_embeddings
                 ORDER BY score DESC
                 LIMIT {}",
                limit
            )
        };

        let mut result = if let Some(cid) = company_id {
            self.db
                .query(query)
                .bind(("query_vector", query_vector))
                .bind(("company_id", cid.to_string()))
                .await
                .context("Failed to execute search query")?
        } else {
            self.db
                .query(query)
                .bind(("query_vector", query_vector))
                .await
                .context("Failed to execute search query")?
        };

        // Parse results
        #[derive(Deserialize)]
        struct RawSearchResult {
            embedding_id: String,
            document_id: String,
            company_id: String,
            deal_id: Option<String>,
            chunk_text: String,
            chunk_index: usize,
            chunk_position: usize,
            page_number: Option<i32>,
            score: f32,
        }

        let raw_results: Vec<RawSearchResult> =
            result.take(0).context("Failed to parse search results")?;

        let search_results: Vec<SearchResult> = raw_results
            .into_iter()
            .filter_map(|r| {
                Some(SearchResult {
                    embedding_id: Uuid::parse_str(&r.embedding_id).ok()?,
                    document_id: Uuid::parse_str(&r.document_id).ok()?,
                    company_id: Uuid::parse_str(&r.company_id).ok()?,
                    deal_id: r.deal_id.and_then(|id| Uuid::parse_str(&id).ok()),
                    score: r.score,
                    chunk_text: r.chunk_text,
                    chunk_index: r.chunk_index,
                    chunk_position: r.chunk_position,
                    page_number: r.page_number,
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

        let query = format!(
            "SELECT embedding_id, document_id, company_id, deal_id, chunk_text,
             chunk_index, chunk_position, page_number,
             vector::similarity::cosine(embedding, $query_vector) AS score
             FROM document_embeddings
             WHERE document_id = $document_id
             ORDER BY score DESC
             LIMIT {}",
            limit
        );

        let mut result = self
            .db
            .query(query)
            .bind(("query_vector", query_vector))
            .bind(("document_id", document_id.to_string()))
            .await
            .context("Failed to execute search query")?;

        // Parse results
        #[derive(Deserialize)]
        struct RawSearchResult {
            embedding_id: String,
            document_id: String,
            company_id: String,
            deal_id: Option<String>,
            chunk_text: String,
            chunk_index: usize,
            chunk_position: usize,
            page_number: Option<i32>,
            score: f32,
        }

        let raw_results: Vec<RawSearchResult> =
            result.take(0).context("Failed to parse search results")?;

        let search_results: Vec<SearchResult> = raw_results
            .into_iter()
            .filter_map(|r| {
                Some(SearchResult {
                    embedding_id: Uuid::parse_str(&r.embedding_id).ok()?,
                    document_id: Uuid::parse_str(&r.document_id).ok()?,
                    company_id: Uuid::parse_str(&r.company_id).ok()?,
                    deal_id: r.deal_id.and_then(|id| Uuid::parse_str(&id).ok()),
                    score: r.score,
                    chunk_text: r.chunk_text,
                    chunk_index: r.chunk_index,
                    chunk_position: r.chunk_position,
                    page_number: r.page_number,
                })
            })
            .collect();

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
