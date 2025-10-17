use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::csv_extractor::CsvExtractor;
use super::embedding_service::EmbeddingService;
use super::excel_extractor::ExcelExtractor;
use super::job_queue_worker::JobProcessor;
use super::pdf_extractor::PdfExtractor;
use super::surrealdb_service::{Document, DocumentEmbedding, SurrealDBService};
use super::text_chunker::TextChunker;
use chrono::Utc;
use uuid::Uuid;

/// Ingestion orchestrator that implements the full document processing pipeline
///
/// Pipeline stages:
/// 1. Parse document (PDF/Excel/CSV)
/// 2. Chunk text intelligently
/// 3. Generate embeddings
/// 4. Store embeddings in SurrealDB
pub struct IngestionOrchestrator {
    db: Arc<SurrealDBService>,
    embedding_service: Arc<EmbeddingService>,
    pdf_extractor: PdfExtractor,
    excel_extractor: ExcelExtractor,
    csv_extractor: CsvExtractor,
    text_chunker: TextChunker,
}

impl IngestionOrchestrator {
    /// Create a new ingestion orchestrator
    pub fn new(db: Arc<SurrealDBService>, embedding_service: Arc<EmbeddingService>) -> Self {
        Self {
            db,
            embedding_service,
            pdf_extractor: PdfExtractor::new(),
            excel_extractor: ExcelExtractor,
            csv_extractor: CsvExtractor::new(),
            text_chunker: TextChunker::default_semantic(),
        }
    }

    /// Process a document through the full pipeline
    async fn process_document_internal(&self, document: &Document) -> Result<()> {
        let document_id = document.document_id;
        let filename = &document.filename;

        info!(
            "Starting ingestion pipeline for document {} ({})",
            document_id, filename
        );

        // Stage 1: Parse document based on file type
        self.update_progress(document_id, 0.1, "Parsing document")
            .await?;

        let chunks = self
            .parse_and_chunk(document)
            .await
            .context("Failed to parse and chunk document")?;

        info!(
            "Parsed document {} into {} chunks",
            document_id,
            chunks.len()
        );

        if chunks.is_empty() {
            warn!("Document {} produced no chunks", document_id);
            return Ok(());
        }

        // Stage 2: Generate embeddings
        self.update_progress(document_id, 0.4, "Generating embeddings")
            .await?;

        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self
            .embedding_service
            .embed_batch_chunked(chunk_texts, 50)
            .await
            .context("Failed to generate embeddings")?;

        info!(
            "Generated {} embeddings for document {}",
            embeddings.len(),
            document_id
        );

        // Stage 3: Store embeddings in SurrealDB
        self.update_progress(document_id, 0.7, "Storing embeddings")
            .await?;

        let document_embeddings: Vec<DocumentEmbedding> = chunks
            .into_iter()
            .zip(embeddings.into_iter())
            .map(|(chunk, embedding)| DocumentEmbedding {
                embedding_id: Uuid::new_v4(),
                document_id: document.document_id,
                company_id: document.company_id,
                deal_id: document.deal_id,
                chunk_index: chunk.chunk_index,
                chunk_text: chunk.text,
                chunk_position: chunk.start_index,
                page_number: chunk.metadata.page_number,
                embedding,
                created_at: Utc::now(),
            })
            .collect();

        self.db
            .create_embeddings_batch(document_embeddings)
            .await
            .context("Failed to store embeddings")?;

        // Stage 4: Complete
        self.update_progress(document_id, 1.0, "Processing complete")
            .await?;

        info!(
            "Successfully completed ingestion pipeline for document {}",
            document_id
        );

        Ok(())
    }

    /// Parse and chunk document based on file type
    async fn parse_and_chunk(
        &self,
        document: &Document,
    ) -> Result<Vec<super::text_chunker::TextChunk>> {
        let filename = &document.filename;
        let extension = filename.rsplit('.').next().unwrap_or("").to_lowercase();

        debug!(
            "Parsing document {} with extension '{}'",
            document.document_id, extension
        );

        match extension.as_str() {
            "pdf" => self.parse_pdf(document).await,
            "xlsx" | "xls" | "xlsm" | "ods" => self.parse_excel(document).await,
            "csv" => self.parse_csv(document).await,
            _ => {
                // Fallback: treat as plain text
                warn!(
                    "Unknown file extension '{}' for document {}, treating as plain text",
                    extension, document.document_id
                );
                self.text_chunker
                    .chunk(&document.content)
                    .context("Failed to chunk plain text")
            }
        }
    }

    /// Parse PDF document
    async fn parse_pdf(&self, document: &Document) -> Result<Vec<super::text_chunker::TextChunk>> {
        debug!("Parsing PDF document {}", document.document_id);

        // Write content to temp file for parsing
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("{}.pdf", document.document_id));

        std::fs::write(&temp_file, &document.content).context("Failed to write temp PDF file")?;

        // Extract text from PDF
        let pdf_result = self
            .pdf_extractor
            .extract_from_file(&temp_file)
            .context("Failed to extract PDF content")?;

        // Clean up temp file
        std::fs::remove_file(&temp_file).ok();

        // Chunk by page
        let chunks = self
            .text_chunker
            .chunk_pdf(&pdf_result.text_by_page)
            .context("Failed to chunk PDF")?;

        Ok(chunks)
    }

    /// Parse Excel document
    async fn parse_excel(
        &self,
        document: &Document,
    ) -> Result<Vec<super::text_chunker::TextChunk>> {
        debug!("Parsing Excel document {}", document.document_id);

        // Write content to temp file for parsing
        let temp_dir = std::env::temp_dir();
        let extension = document.filename.rsplit('.').next().unwrap_or("xlsx");
        let temp_file = temp_dir.join(format!("{}.{}", document.document_id, extension));

        std::fs::write(&temp_file, &document.content).context("Failed to write temp Excel file")?;

        // Extract data from Excel
        let excel_result = self
            .excel_extractor
            .extract_from_file(&temp_file)
            .context("Failed to extract Excel content")?;

        // Clean up temp file
        std::fs::remove_file(&temp_file).ok();

        // Chunk each sheet separately
        let mut all_chunks = Vec::new();
        for sheet in &excel_result.sheets {
            let sheet_chunks = self
                .text_chunker
                .chunk_excel(&sheet.name, &sheet.rows, 50)
                .context("Failed to chunk Excel sheet")?;
            all_chunks.extend(sheet_chunks);
        }

        Ok(all_chunks)
    }

    /// Parse CSV document
    async fn parse_csv(&self, document: &Document) -> Result<Vec<super::text_chunker::TextChunk>> {
        debug!("Parsing CSV document {}", document.document_id);

        // Write content to temp file for parsing
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("{}.csv", document.document_id));

        std::fs::write(&temp_file, &document.content).context("Failed to write temp CSV file")?;

        // Extract data from CSV
        let csv_result = self
            .csv_extractor
            .extract_from_file(&temp_file)
            .context("Failed to extract CSV content")?;

        // Clean up temp file
        std::fs::remove_file(&temp_file).ok();

        // Chunk with headers preserved
        let chunks = self
            .text_chunker
            .chunk_csv(&csv_result.data.headers, &csv_result.data.rows, 50)
            .context("Failed to chunk CSV")?;

        Ok(chunks)
    }

    /// Update document processing progress
    async fn update_progress(&self, document_id: Uuid, progress: f32, message: &str) -> Result<()> {
        debug!(
            "Document {} progress: {:.0}% - {}",
            document_id,
            progress * 100.0,
            message
        );

        self.db
            .update_document_status(document_id, "processing", Some(progress), None)
            .await
            .context("Failed to update document progress")?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl JobProcessor for IngestionOrchestrator {
    async fn process_document(&self, document: &Document) -> Result<()> {
        self.process_document_internal(document).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension_detection() {
        let extensions = vec![
            ("test.pdf", "pdf"),
            ("document.xlsx", "xlsx"),
            ("data.csv", "csv"),
            ("file.XLS", "xls"),
            ("report.PDF", "pdf"),
        ];

        for (filename, expected) in extensions {
            let ext = filename.rsplit('.').next().unwrap().to_lowercase();
            assert_eq!(ext, expected);
        }
    }
}
