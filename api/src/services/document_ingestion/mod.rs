//! Document Ingestion Services
//!
//! This module contains all services related to document ingestion and processing:
//! - PDF text extraction
//! - Excel/XLS text extraction
//! - CSV text extraction
//! - Text chunking for embeddings with metadata preservation
//! - Vector storage (SurrealDB)
//! - Embedding generation (via rig and Modal)
//! - Document field extraction (via native Rust LLM integration)
//! - Comprehensive extraction schemas matching MVP CSV specifications
//! - Ingestion orchestration

pub mod comprehensive_schema;
pub mod csv_extractor;
pub mod embedding_service;
pub mod excel_extractor;
pub mod llm_extraction;
pub mod pdf_extractor;
pub mod surrealdb_service;
pub mod text_chunker;

// Re-export commonly used types
pub use csv_extractor::{CsvData, CsvExtractionResult, CsvExtractor, CsvMetadata};
pub use embedding_service::{cosine_similarity, EmbeddingModel, EmbeddingService};
pub use excel_extractor::{ExcelExtractionResult, ExcelExtractor, ExcelMetadata, ExcelSheet};
pub use llm_extraction::{
    CollateralType, CurrencyCode, DealCollateral, DealCovenants, DealFinancialTerms, DealParties,
    DealType, DocumentMetadata, ExtractedDealData, InterestRateType, LlmExtractionService,
    PaymentFrequency,
};
pub use pdf_extractor::{PdfExtractionResult, PdfExtractor, PdfMetadata};
pub use surrealdb_service::{Document, DocumentEmbedding, SearchResult, SurrealDBService};
pub use text_chunker::{ChunkMetadata, ChunkingStats, ChunkingStrategy, TextChunk, TextChunker};

// Re-export comprehensive schema types
pub use comprehensive_schema::{
    ComprehensiveExtractionResult, CreditAgreementExtraction, DetectedDocumentType, DocumentData,
    DocumentationStandard, ExecutionStatus, FacilityType, ReferenceRate, RepaymentType,
    SecurityAgreementExtraction, TermSheetExtraction, TermSheetStatus, TestFrequency,
};
