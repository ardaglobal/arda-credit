//! Document Ingestion Services
//!
//! This module contains all services related to document ingestion and processing:
//! - PDF text extraction
//! - Text chunking for embeddings
//! - Vector storage (SurrealDB)
//! - Embedding generation
//! - Document field extraction (via native Rust LLM integration)
//! - Comprehensive extraction schemas matching MVP CSV specifications
//! - Ingestion orchestration

pub mod comprehensive_schema;
pub mod embedding_service;
pub mod llm_extraction;
pub mod pdf_extractor;
pub mod surrealdb_service;
pub mod text_chunker;

// Re-export commonly used types
pub use embedding_service::{cosine_similarity, EmbeddingModel, EmbeddingService};
pub use llm_extraction::{
    CollateralType, CurrencyCode, DealCollateral, DealCovenants, DealFinancialTerms, DealParties,
    DealType, DocumentMetadata, ExtractedDealData, InterestRateType, LlmExtractionService,
    PaymentFrequency,
};
pub use pdf_extractor::{PdfExtractionResult, PdfExtractor, PdfMetadata};
pub use surrealdb_service::{Document, DocumentEmbedding, SearchResult, SurrealDBService};
pub use text_chunker::{ChunkingStats, ChunkingStrategy, TextChunk, TextChunker};

// Re-export comprehensive schema types
pub use comprehensive_schema::{
    ComprehensiveExtractionResult, CreditAgreementExtraction, DetectedDocumentType, DocumentData,
    DocumentationStandard, ExecutionStatus, FacilityType, ReferenceRate, RepaymentType,
    SecurityAgreementExtraction, TermSheetExtraction, TermSheetStatus, TestFrequency,
};
