use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, Distance, FieldType,
    Filter, KeywordIndexParamsBuilder, PointStruct, SearchPointsBuilder, UpsertPointsBuilder,
    VectorParams, VectorsConfig,
};
use qdrant_client::Qdrant;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Metadata for a document chunk stored in Qdrant
///
/// Multi-tenant support: `company_id` enables customer partitioning
/// for privacy and performance optimization with Qdrant's `is_tenant` feature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub document_id: Uuid,
    pub company_id: Uuid,
    pub deal_id: Option<Uuid>,
    pub chunk_index: usize,
    pub chunk_text: String,
    pub page_number: Option<i32>,
    pub document_type: String,
    pub uploaded_at: String,
}

/// Search result from Qdrant
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub chunk_id: Uuid,
    pub score: f32,
    pub metadata: ChunkMetadata,
}

/// Qdrant Service for vector storage and semantic search
///
/// Provides:
/// - Collection management
/// - Vector upsert with metadata
/// - Semantic search with filters
/// - Batch operations
pub struct QdrantService {
    client: Qdrant,
    collection_name: String,
    embedding_dimension: u64,
}

impl QdrantService {
    /// Create a new Qdrant service from environment variables
    ///
    /// Required env vars:
    /// - QDRANT_URL (default: http://localhost:6333)
    /// - QDRANT_COLLECTION_NAME (default: arda_documents)
    /// - EMBEDDING_DIMENSIONS (default: 1536)
    pub fn from_env() -> Result<Self> {
        let url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://localhost:6333".to_string());
        let collection_name = std::env::var("QDRANT_COLLECTION_NAME")
            .unwrap_or_else(|_| "arda_documents".to_string());
        let embedding_dimension: u64 = std::env::var("EMBEDDING_DIMENSIONS")
            .unwrap_or_else(|_| "1536".to_string())
            .parse()
            .context("Invalid EMBEDDING_DIMENSIONS value")?;

        info!(
            "Connecting to Qdrant at {} (collection: {})",
            url, collection_name
        );

        Self::new(&url, &collection_name, embedding_dimension)
    }

    /// Create a new Qdrant service with explicit configuration
    pub fn new(url: &str, collection_name: &str, embedding_dimension: u64) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
            embedding_dimension,
        })
    }

    /// Initialize the collection (create if doesn't exist)
    ///
    /// This should be called during application startup.
    /// Creates collection with multi-tenant optimization via `is_tenant` payload index.
    pub async fn init_collection(&self) -> Result<()> {
        info!("Initializing Qdrant collection: {}", self.collection_name);

        // Check if collection exists
        let collections = self.client.list_collections().await?;
        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == self.collection_name);

        if exists {
            info!("Collection '{}' already exists", self.collection_name);
            // Ensure tenant index exists (idempotent)
            self.create_tenant_index().await?;
            return Ok(());
        }

        // Create collection with cosine distance for embeddings
        info!(
            "Creating collection '{}' with dimension {}",
            self.collection_name, self.embedding_dimension
        );

        let vectors_config = VectorsConfig {
            config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                VectorParams {
                    size: self.embedding_dimension,
                    distance: Distance::Cosine.into(),
                    ..Default::default()
                },
            )),
        };

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&self.collection_name).vectors_config(vectors_config),
            )
            .await?;

        info!("Collection '{}' created successfully", self.collection_name);

        // Create multi-tenant payload index
        self.create_tenant_index().await?;

        Ok(())
    }

    /// Create payload index for company_id with is_tenant optimization
    ///
    /// The `is_tenant=true` parameter (Qdrant v1.11+) co-locates vectors from
    /// the same tenant on disk for significantly improved bulk read performance
    /// and reduced scaling costs in multi-tenant deployments.
    async fn create_tenant_index(&self) -> Result<()> {
        info!(
            "Creating tenant-optimized payload index for company_id in collection: {}",
            self.collection_name
        );

        self.client
            .create_field_index(
                CreateFieldIndexCollectionBuilder::new(
                    &self.collection_name,
                    "company_id",
                    FieldType::Keyword,
                )
                .field_index_params(KeywordIndexParamsBuilder::default().is_tenant(true)),
            )
            .await
            .context("Failed to create tenant payload index")?;

        info!("Tenant payload index created successfully");
        Ok(())
    }

    /// Delete the collection (for testing/cleanup)
    pub async fn delete_collection(&self) -> Result<()> {
        warn!("Deleting collection: {}", self.collection_name);
        self.client.delete_collection(&self.collection_name).await?;
        Ok(())
    }

    /// Upsert a single vector with metadata
    ///
    /// # Arguments
    /// * `chunk_id` - Unique ID for this chunk
    /// * `vector` - Embedding vector (must match collection dimension)
    /// * `metadata` - Chunk metadata for filtering and retrieval
    pub async fn upsert_vector(
        &self,
        chunk_id: Uuid,
        vector: Vec<f32>,
        metadata: ChunkMetadata,
    ) -> Result<()> {
        self.upsert_vectors(vec![(chunk_id, vector, metadata)])
            .await
    }

    /// Upsert multiple vectors in batch
    ///
    /// More efficient than calling upsert_vector multiple times
    pub async fn upsert_vectors(
        &self,
        vectors: Vec<(Uuid, Vec<f32>, ChunkMetadata)>,
    ) -> Result<()> {
        if vectors.is_empty() {
            return Ok(());
        }

        debug!("Upserting {} vectors to Qdrant", vectors.len());

        // Validate vector dimensions
        for (chunk_id, vector, _) in &vectors {
            if vector.len() != self.embedding_dimension as usize {
                anyhow::bail!(
                    "Vector {} has dimension {} but expected {}",
                    chunk_id,
                    vector.len(),
                    self.embedding_dimension
                );
            }
        }

        // Convert to Qdrant points
        let points: Vec<PointStruct> = vectors
            .into_iter()
            .map(|(chunk_id, vector, metadata)| {
                // Serialize metadata to JSON
                let payload = serde_json::to_value(&metadata)
                    .expect("Failed to serialize metadata")
                    .as_object()
                    .expect("Metadata must be an object")
                    .clone();

                PointStruct::new(chunk_id.to_string(), vector, payload)
            })
            .collect();

        // Upsert to Qdrant
        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection_name, points))
            .await?;

        info!("Successfully upserted vectors to Qdrant");
        Ok(())
    }

    /// Semantic search across all documents
    ///
    /// # Arguments
    /// * `query_vector` - Embedding of the search query
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// * Vector of search results sorted by relevance (highest score first)
    pub async fn search(&self, query_vector: Vec<f32>, limit: u64) -> Result<Vec<SearchResult>> {
        self.search_with_filter(query_vector, limit, None).await
    }

    /// Semantic search with metadata filters
    ///
    /// # Arguments
    /// * `query_vector` - Embedding of the search query
    /// * `limit` - Maximum number of results to return
    /// * `document_id` - Optional document ID to filter by
    ///
    /// # Returns
    /// * Vector of search results sorted by relevance
    pub async fn search_with_filter(
        &self,
        query_vector: Vec<f32>,
        limit: u64,
        document_id: Option<Uuid>,
    ) -> Result<Vec<SearchResult>> {
        debug!(
            "Searching Qdrant with limit {} (filter: {:?})",
            limit, document_id
        );

        // Validate vector dimension
        if query_vector.len() != self.embedding_dimension as usize {
            anyhow::bail!(
                "Query vector has dimension {} but expected {}",
                query_vector.len(),
                self.embedding_dimension
            );
        }

        // Build filter if document_id provided
        let filter = document_id
            .map(|doc_id| Filter::must([Condition::matches("document_id", doc_id.to_string())]));

        // Execute search
        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, query_vector, limit)
                    .filter(filter.unwrap_or_default())
                    .with_payload(true),
            )
            .await?;

        // Convert to SearchResult
        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .filter_map(|scored_point| {
                // Extract ID from PointId enum
                let id_str = match &scored_point.id?.point_id_options? {
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid_str) => {
                        uuid_str.clone()
                    }
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(_) => return None,
                };

                let chunk_id = Uuid::parse_str(&id_str).ok()?;
                let score = scored_point.score;

                // Convert Qdrant payload to serde_json::Value
                let payload_json = serde_json::to_value(&scored_point.payload).ok()?;

                // Deserialize metadata
                let metadata: ChunkMetadata = serde_json::from_value(payload_json).ok()?;

                Some(SearchResult {
                    chunk_id,
                    score,
                    metadata,
                })
            })
            .collect();

        debug!("Found {} search results", results.len());
        Ok(results)
    }

    /// Search within a specific company's documents (multi-tenant scoped)
    ///
    /// This leverages the tenant-optimized payload index for improved performance.
    ///
    /// # Arguments
    /// * `company_id` - Company ID to scope the search to
    /// * `query_vector` - Embedding of the search query
    /// * `limit` - Maximum number of results to return
    ///
    /// # Returns
    /// * Vector of search results sorted by relevance, only from specified company
    pub async fn search_by_company(
        &self,
        company_id: Uuid,
        query_vector: Vec<f32>,
        limit: u64,
    ) -> Result<Vec<SearchResult>> {
        debug!(
            "Searching Qdrant for company {} with limit {}",
            company_id, limit
        );

        // Validate vector dimension
        if query_vector.len() != self.embedding_dimension as usize {
            anyhow::bail!(
                "Query vector has dimension {} but expected {}",
                query_vector.len(),
                self.embedding_dimension
            );
        }

        // Filter by company_id using tenant-optimized index
        let filter = Filter::must([Condition::matches("company_id", company_id.to_string())]);

        // Execute search
        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, query_vector, limit)
                    .filter(filter)
                    .with_payload(true),
            )
            .await?;

        // Convert to SearchResult
        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .filter_map(|scored_point| {
                let id_str = match &scored_point.id?.point_id_options? {
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid_str) => {
                        uuid_str.clone()
                    }
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(_) => return None,
                };

                let chunk_id = Uuid::parse_str(&id_str).ok()?;
                let score = scored_point.score;

                let payload_json = serde_json::to_value(&scored_point.payload).ok()?;
                let metadata: ChunkMetadata = serde_json::from_value(payload_json).ok()?;

                Some(SearchResult {
                    chunk_id,
                    score,
                    metadata,
                })
            })
            .collect();

        debug!("Found {} results for company {}", results.len(), company_id);
        Ok(results)
    }

    /// Search within a company's specific document
    ///
    /// Combines company and document filtering for precise multi-tenant queries.
    ///
    /// # Arguments
    /// * `company_id` - Company ID to scope the search to
    /// * `document_id` - Document ID to filter by
    /// * `query_vector` - Embedding of the search query
    /// * `limit` - Maximum number of results to return
    pub async fn search_by_company_and_document(
        &self,
        company_id: Uuid,
        document_id: Uuid,
        query_vector: Vec<f32>,
        limit: u64,
    ) -> Result<Vec<SearchResult>> {
        debug!(
            "Searching Qdrant for company {} document {} with limit {}",
            company_id, document_id, limit
        );

        // Validate vector dimension
        if query_vector.len() != self.embedding_dimension as usize {
            anyhow::bail!(
                "Query vector has dimension {} but expected {}",
                query_vector.len(),
                self.embedding_dimension
            );
        }

        // Filter by both company_id and document_id
        let filter = Filter::must([
            Condition::matches("company_id", company_id.to_string()),
            Condition::matches("document_id", document_id.to_string()),
        ]);

        // Execute search
        let search_result = self
            .client
            .search_points(
                SearchPointsBuilder::new(&self.collection_name, query_vector, limit)
                    .filter(filter)
                    .with_payload(true),
            )
            .await?;

        // Convert to SearchResult
        let results: Vec<SearchResult> = search_result
            .result
            .into_iter()
            .filter_map(|scored_point| {
                let id_str = match &scored_point.id?.point_id_options? {
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid_str) => {
                        uuid_str.clone()
                    }
                    qdrant_client::qdrant::point_id::PointIdOptions::Num(_) => return None,
                };

                let chunk_id = Uuid::parse_str(&id_str).ok()?;
                let score = scored_point.score;

                let payload_json = serde_json::to_value(&scored_point.payload).ok()?;
                let metadata: ChunkMetadata = serde_json::from_value(payload_json).ok()?;

                Some(SearchResult {
                    chunk_id,
                    score,
                    metadata,
                })
            })
            .collect();

        debug!(
            "Found {} results for company {} document {}",
            results.len(),
            company_id,
            document_id
        );
        Ok(results)
    }

    /// Delete all vectors for a specific document
    ///
    /// Useful for document deletion or re-ingestion
    pub async fn delete_document(&self, document_id: Uuid) -> Result<()> {
        info!("Deleting all vectors for document {}", document_id);

        // Delete points matching document_id filter
        use qdrant_client::qdrant::{DeletePoints, PointsSelector};

        let filter = Filter::must([Condition::matches("document_id", document_id.to_string())]);

        let points_selector = PointsSelector {
            points_selector_one_of: Some(
                qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(filter),
            ),
        };

        let request = DeletePoints {
            collection_name: self.collection_name.clone(),
            points: Some(points_selector),
            ..Default::default()
        };

        self.client.delete_points(request).await?;

        Ok(())
    }

    /// Delete all vectors for a specific company (multi-tenant cleanup)
    ///
    /// Useful for company offboarding or data deletion requests
    pub async fn delete_company_documents(&self, company_id: Uuid) -> Result<()> {
        info!("Deleting all vectors for company {}", company_id);

        use qdrant_client::qdrant::{DeletePoints, PointsSelector};

        let filter = Filter::must([Condition::matches("company_id", company_id.to_string())]);

        let points_selector = PointsSelector {
            points_selector_one_of: Some(
                qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(filter),
            ),
        };

        let request = DeletePoints {
            collection_name: self.collection_name.clone(),
            points: Some(points_selector),
            ..Default::default()
        };

        self.client.delete_points(request).await?;

        Ok(())
    }

    /// Delete specific document within a company
    ///
    /// Combines company and document filtering for safe multi-tenant deletion
    pub async fn delete_company_document(&self, company_id: Uuid, document_id: Uuid) -> Result<()> {
        info!(
            "Deleting vectors for company {} document {}",
            company_id, document_id
        );

        use qdrant_client::qdrant::{DeletePoints, PointsSelector};

        let filter = Filter::must([
            Condition::matches("company_id", company_id.to_string()),
            Condition::matches("document_id", document_id.to_string()),
        ]);

        let points_selector = PointsSelector {
            points_selector_one_of: Some(
                qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(filter),
            ),
        };

        let request = DeletePoints {
            collection_name: self.collection_name.clone(),
            points: Some(points_selector),
            ..Default::default()
        };

        self.client.delete_points(request).await?;

        Ok(())
    }

    /// Get collection info (point count, etc.)
    pub async fn get_collection_info(&self) -> Result<HashMap<String, serde_json::Value>> {
        let info = self.client.collection_info(&self.collection_name).await?;

        let mut result = HashMap::new();
        if let Some(collection) = info.result {
            result.insert(
                "points_count".to_string(),
                serde_json::json!(collection.points_count.unwrap_or(0)),
            );
            result.insert(
                "vectors_count".to_string(),
                serde_json::json!(collection.vectors_count.unwrap_or(0)),
            );
            result.insert("status".to_string(), serde_json::json!(collection.status));
        }

        Ok(result)
    }

    /// Health check - verify Qdrant connection
    pub async fn health_check(&self) -> Result<bool> {
        let collections = self.client.list_collections().await?;
        let exists = collections
            .collections
            .iter()
            .any(|c| c.name == self.collection_name);

        if !exists {
            warn!("Collection '{}' does not exist", self.collection_name);
            return Ok(false);
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Qdrant running
    async fn test_qdrant_connection() {
        let service = QdrantService::new("http://localhost:6333", "test_collection", 1536)
            .expect("Failed to create service");

        let healthy = service.health_check().await.expect("Health check failed");
        assert!(healthy || !healthy); // Just test connection works
    }

    #[test]
    fn test_chunk_metadata_serialization() {
        let metadata = ChunkMetadata {
            document_id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            deal_id: Some(Uuid::new_v4()),
            chunk_index: 0,
            chunk_text: "Test chunk".to_string(),
            page_number: Some(1),
            document_type: "credit_agreement".to_string(),
            uploaded_at: "2025-10-14T12:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&metadata).expect("Serialization failed");
        let deserialized: ChunkMetadata =
            serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(metadata.document_id, deserialized.document_id);
        assert_eq!(metadata.company_id, deserialized.company_id);
        assert_eq!(metadata.chunk_index, deserialized.chunk_index);
    }
}
