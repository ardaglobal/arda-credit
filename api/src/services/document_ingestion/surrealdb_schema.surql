-- SurrealDB Schema for Arda Credit Document Ingestion
-- Multi-tenant document storage with vector embeddings

-- Documents Table
-- Stores document metadata and content
DEFINE TABLE documents SCHEMAFULL;

DEFINE FIELD document_id ON documents TYPE record<documents>;
DEFINE FIELD company_id ON documents TYPE record<companies>;
DEFINE FIELD deal_id ON documents TYPE option<record<deals>>;
DEFINE FIELD filename ON documents TYPE string;
DEFINE FIELD document_type ON documents TYPE string;
DEFINE FIELD file_size ON documents TYPE int;
DEFINE FIELD user_id ON documents TYPE option<record<users>>;
DEFINE FIELD upload_timestamp ON documents TYPE datetime DEFAULT time::now();
DEFINE FIELD status ON documents TYPE string DEFAULT 'uploaded';
    -- Status options: 'uploaded', 'queued', 'processing', 'completed', 'failed'
DEFINE FIELD content ON documents TYPE string;
DEFINE FIELD page_count ON documents TYPE option<int>;
DEFINE FIELD processing_progress ON documents TYPE option<float> DEFAULT 0.0;
    -- Progress percentage (0.0 to 1.0)
DEFINE FIELD error_message ON documents TYPE option<string>;
    -- Error details if status is 'failed'
DEFINE FIELD retry_count ON documents TYPE int DEFAULT 0;
    -- Number of retry attempts
DEFINE FIELD processing_started_at ON documents TYPE option<datetime>;
    -- When processing began
DEFINE FIELD processing_completed_at ON documents TYPE option<datetime>;
    -- When processing completed (success or failure)
DEFINE FIELD created_at ON documents TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON documents TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX idx_document_company ON documents FIELDS company_id;
DEFINE INDEX idx_document_deal ON documents FIELDS deal_id;
DEFINE INDEX idx_document_user ON documents FIELDS user_id;
DEFINE INDEX idx_document_status ON documents FIELDS status;
DEFINE INDEX idx_document_type ON documents FIELDS document_type;

-- Document Embeddings Table
-- Stores vector embeddings for semantic search
DEFINE TABLE document_embeddings SCHEMAFULL;

DEFINE FIELD embedding_id ON document_embeddings TYPE record<document_embeddings>;
DEFINE FIELD document_id ON document_embeddings TYPE record<documents>;
DEFINE FIELD company_id ON document_embeddings TYPE record<companies>;
DEFINE FIELD deal_id ON document_embeddings TYPE option<record<deals>>;
DEFINE FIELD user_id ON document_embeddings TYPE option<record<users>>;
    -- User who uploaded the document (for filtering)
DEFINE FIELD chunk_index ON document_embeddings TYPE int;
DEFINE FIELD chunk_text ON document_embeddings TYPE string;
DEFINE FIELD chunk_position ON document_embeddings TYPE int;
DEFINE FIELD page_number ON document_embeddings TYPE option<int>;
DEFINE FIELD embedding ON document_embeddings TYPE array<float>;
    -- Vector embedding (1536 dimensions for text-embedding-3-small)
DEFINE FIELD created_at ON document_embeddings TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX idx_embedding_document ON document_embeddings FIELDS document_id;
DEFINE INDEX idx_embedding_company ON document_embeddings FIELDS company_id;
DEFINE INDEX idx_embedding_deal ON document_embeddings FIELDS deal_id;
DEFINE INDEX idx_embedding_user ON document_embeddings FIELDS user_id;

-- MTREE Index for fast vector similarity search
-- Uses cosine similarity for semantic search
DEFINE INDEX idx_embedding_vector ON document_embeddings FIELDS embedding MTREE DIMENSION 1536 DIST COSINE;

-- Full-text search index for hybrid search (keyword search)
DEFINE INDEX idx_embedding_fulltext ON document_embeddings FIELDS chunk_text SEARCH ANALYZER ascii BM25;
