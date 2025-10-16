//! Document Ingestion Orchestrator
//!
//! This module orchestrates the complete document ingestion pipeline with multi-tenant support:
//! 1. Extract text from PDF/documents
//! 2. Chunk text for optimal embedding
//! 3. Generate embeddings via OpenAI/Anthropic
//! 4. Store vectors in Qdrant with company_id partitioning
//! 5. Extract structured data via LLM
//! 6. Update database with ingestion status

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

use super::document_ingestion::{
    ChunkMetadata, EmbeddingService, LlmExtractionService, PdfExtractor, QdrantService, TextChunker,
};

/// Document ingestion orchestrator with multi-tenant support
pub struct DocumentIngestionOrchestrator {
    pdf_extractor: PdfExtractor,
    text_chunker: TextChunker,
    embedding_service: EmbeddingService,
    qdrant_service: QdrantService,
    llm_extraction_service: LlmExtractionService,
}

/// Document ingestion request
#[derive(Debug, Clone)]
pub struct DocumentIngestionRequest {
    pub document_id: Uuid,
    pub company_id: Uuid,
    pub deal_id: Option<Uuid>,
    pub document_type: String,
    pub file_path: String,
}

/// Document ingestion result
#[derive(Debug, Clone)]
pub struct DocumentIngestionResult {
    pub document_id: Uuid,
    pub chunks_processed: usize,
    pub vectors_stored: usize,
    pub extracted_data: Option<serde_json::Value>,
    pub processing_time_ms: u128,
}

impl DocumentIngestionOrchestrator {
    /// Create a new document ingestion orchestrator from environment
    pub async fn from_env(_db_pool: PgPool) -> Result<Self> {
        info!("Initializing Document Ingestion Orchestrator");

        let pdf_extractor = PdfExtractor::new();
        let text_chunker = TextChunker::new(
            crate::services::document_ingestion::ChunkingStrategy::Semantic {
                target_size: 512,
                overlap: 50,
            },
        );
        let embedding_service =
            EmbeddingService::from_env().context("Failed to initialize embedding service")?;
        let qdrant_service =
            QdrantService::from_env().context("Failed to initialize Qdrant service")?;
        let llm_extraction_service = LlmExtractionService::from_env()
            .context("Failed to initialize LLM extraction service")?;

        // Initialize Qdrant collection with multi-tenant index
        qdrant_service
            .init_collection()
            .await
            .context("Failed to initialize Qdrant collection")?;

        info!("Document Ingestion Orchestrator initialized successfully");

        Ok(Self {
            pdf_extractor,
            text_chunker,
            embedding_service,
            qdrant_service,
            llm_extraction_service,
        })
    }

    /// Ingest a document with full pipeline (multi-tenant aware)
    #[instrument(skip(self), fields(document_id = %request.document_id, company_id = %request.company_id))]
    pub async fn ingest_document(
        &self,
        request: DocumentIngestionRequest,
    ) -> Result<DocumentIngestionResult> {
        let start_time = std::time::Instant::now();

        info!(
            document_id = %request.document_id,
            company_id = %request.company_id,
            document_type = %request.document_type,
            "Starting document ingestion"
        );

        // Update status to processing
        self.update_ingestion_status(request.document_id, "processing", None)
            .await?;

        // Step 1: Extract text from PDF
        let extraction_result = match self.pdf_extractor.extract_from_file(&request.file_path) {
            Ok(result) => result,
            Err(e) => {
                error!(
                    document_id = %request.document_id,
                    error = %e,
                    "PDF extraction failed"
                );
                self.update_ingestion_status(
                    request.document_id,
                    "failed",
                    Some(format!("PDF extraction failed: {}", e)),
                )
                .await?;
                return Err(e.into());
            }
        };

        info!(
            document_id = %request.document_id,
            pages = extraction_result.metadata.page_count,
            "PDF extracted successfully"
        );

        // Step 2: Chunk text for embedding
        let chunks = self
            .text_chunker
            .chunk(&extraction_result.text)
            .context("Failed to chunk text")?;

        info!(
            document_id = %request.document_id,
            chunk_count = chunks.len(),
            "Text chunked successfully"
        );

        // Step 3: Generate embeddings
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self
            .embedding_service
            .embed_batch_chunked(chunk_texts, 100)
            .await
            .context("Failed to generate embeddings")?;

        info!(
            document_id = %request.document_id,
            embedding_count = embeddings.len(),
            "Embeddings generated successfully"
        );

        // Step 4: Store vectors in Qdrant with company_id (multi-tenant)
        let vectors_to_upsert: Vec<(Uuid, Vec<f32>, ChunkMetadata)> = chunks
            .iter()
            .zip(embeddings.iter())
            .enumerate()
            .map(|(i, (chunk, embedding))| {
                let chunk_id = Uuid::new_v4();
                let metadata = ChunkMetadata {
                    document_id: request.document_id,
                    company_id: request.company_id, // ⭐ Multi-tenant partitioning
                    deal_id: request.deal_id,
                    chunk_index: i,
                    chunk_text: chunk.text.clone(),
                    page_number: None, // TextChunk doesn't have page_number field
                    document_type: request.document_type.clone(),
                    uploaded_at: Utc::now().to_rfc3339(),
                };
                (chunk_id, embedding.clone(), metadata)
            })
            .collect();

        self.qdrant_service
            .upsert_vectors(vectors_to_upsert)
            .await
            .context("Failed to upsert vectors to Qdrant")?;

        info!(
            document_id = %request.document_id,
            company_id = %request.company_id,
            vectors_stored = chunks.len(),
            "Vectors stored in Qdrant with company partitioning"
        );

        // Step 5: Extract structured data with LLM (optional, async)
        let extracted_data = match request.document_type.as_str() {
            "credit_agreement" | "term_sheet" | "loan_agreement" => {
                info!(
                    document_id = %request.document_id,
                    document_type = %request.document_type,
                    "Starting structured data extraction"
                );

                match self
                    .llm_extraction_service
                    .extract_deal(&extraction_result.text)
                    .await
                {
                    Ok(data) => {
                        info!(
                            document_id = %request.document_id,
                            confidence = data.confidence_score,
                            "Structured data extracted successfully"
                        );
                        Some(serde_json::to_value(data).ok())
                    }
                    Err(e) => {
                        warn!(
                            document_id = %request.document_id,
                            error = %e,
                            "LLM extraction failed, continuing without structured data"
                        );
                        None
                    }
                }
            }
            _ => {
                debug!(
                    document_id = %request.document_id,
                    document_type = %request.document_type,
                    "Skipping LLM extraction for document type"
                );
                None
            }
        };

        // Step 6: Update database with completion status
        self.update_ingestion_status(request.document_id, "completed", None)
            .await?;

        if let Some(data) = extracted_data.as_ref().and_then(|d| d.as_ref()) {
            self.save_extracted_data(request.document_id, data).await?;
        }

        let processing_time = start_time.elapsed();

        info!(
            document_id = %request.document_id,
            company_id = %request.company_id,
            processing_time_ms = processing_time.as_millis(),
            chunks = chunks.len(),
            "Document ingestion completed successfully"
        );

        Ok(DocumentIngestionResult {
            document_id: request.document_id,
            chunks_processed: chunks.len(),
            vectors_stored: chunks.len(),
            extracted_data: extracted_data.flatten(),
            processing_time_ms: processing_time.as_millis(),
        })
    }

    /// Process pending ingestion jobs (background worker)
    #[instrument(skip(self))]
    pub async fn process_pending_jobs(&self, _max_jobs: i64) -> Result<usize> {
        // TODO: Implement background job processing
        // This requires:
        // 1. Adding ingestion_jobs table to database
        // 2. Implementing job queue system
        // 3. Background worker to process jobs

        // For now, return 0 processed jobs
        debug!("Background job processing not yet implemented");
        Ok(0)
    }

    /// Update document ingestion status in database
    async fn update_ingestion_status(
        &self,
        _document_id: Uuid,
        _status: &str,
        _error: Option<String>,
    ) -> Result<()> {
        // TODO: Implement database status updates
        // This requires adding ingestion_status fields to documents table
        // For now, this is a no-op placeholder
        Ok(())
    }

    /// Save extracted structured data
    async fn save_extracted_data(
        &self,
        document_id: Uuid,
        _data: &serde_json::Value,
    ) -> Result<()> {
        // TODO: Implement saving extracted data to database
        // This could be stored in a separate table or as JSONB in documents table
        debug!(
            document_id = %document_id,
            "Extracted data saved (placeholder)"
        );
        Ok(())
    }

    /// Health check for ingestion services
    pub async fn health_check(&self) -> Result<bool> {
        // Check Qdrant
        let qdrant_healthy = self.qdrant_service.health_check().await?;
        if !qdrant_healthy {
            warn!("Qdrant health check failed");
            return Ok(false);
        }

        // Check embedding service
        let embedding_healthy = self.embedding_service.health_check().await?;
        if !embedding_healthy {
            warn!("Embedding service health check failed");
            return Ok(false);
        }

        // Check LLM service
        let llm_healthy = self.llm_extraction_service.health_check().await?;
        if !llm_healthy {
            warn!("LLM extraction service health check failed");
            return Ok(false);
        }

        info!("All ingestion services healthy");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingestion_request_creation() {
        let request = DocumentIngestionRequest {
            document_id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            deal_id: Some(Uuid::new_v4()),
            document_type: "credit_agreement".to_string(),
            file_path: "/path/to/document.pdf".to_string(),
        };

        assert_eq!(request.document_type, "credit_agreement");
        assert!(request.deal_id.is_some());
    }
}
