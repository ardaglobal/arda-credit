-- ============================================================================
-- Migration: Add Deal Documents Support
-- ============================================================================
-- This migration adds comprehensive document storage capabilities for deals.
-- Supports PDFs, DOCX, XLSX, and other file types with metadata tracking,
-- versioning, and ingestion status management.
-- ============================================================================

-- ============================================================================
-- STEP 1: Create deal_documents table
-- ============================================================================

CREATE TABLE deal_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    deal_id UUID NOT NULL REFERENCES deals(id) ON DELETE CASCADE,

    -- File metadata
    file_name VARCHAR(255) NOT NULL,
    original_file_name VARCHAR(255) NOT NULL, -- Preserve original filename
    file_type VARCHAR(100) NOT NULL, -- MIME type (e.g., application/pdf)
    file_extension VARCHAR(10) NOT NULL, -- e.g., .pdf, .docx, .xlsx
    file_size_bytes BIGINT NOT NULL CHECK (file_size_bytes > 0),

    -- Storage location
    storage_path TEXT NOT NULL, -- Path to file in storage (S3 key or file system path)
    storage_type VARCHAR(20) NOT NULL DEFAULT 'filesystem' CHECK (storage_type IN ('filesystem', 's3')),

    -- Document categorization
    document_type VARCHAR(50), -- e.g., 'financial_statement', 'contract', 'invoice', 'other'
    document_category VARCHAR(50), -- e.g., 'legal', 'financial', 'operational'
    description TEXT,

    -- Upload information
    uploaded_by UUID NOT NULL REFERENCES users(id),
    uploaded_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Ingestion status
    ingestion_status VARCHAR(20) NOT NULL DEFAULT 'pending'
        CHECK (ingestion_status IN ('pending', 'processing', 'completed', 'failed', 'skipped')),
    ingested_at TIMESTAMP WITH TIME ZONE,
    ingestion_error TEXT, -- Store error message if ingestion fails

    -- Extracted metadata (populated after ingestion)
    page_count INTEGER, -- For PDFs
    word_count INTEGER, -- For documents
    sheet_count INTEGER, -- For spreadsheets
    extracted_text_preview TEXT, -- First N characters of extracted text

    -- Security and validation
    checksum VARCHAR(64) NOT NULL, -- SHA256 hash of file contents for integrity
    is_deleted BOOLEAN DEFAULT FALSE,
    deleted_at TIMESTAMP WITH TIME ZONE,
    deleted_by UUID REFERENCES users(id),

    -- Version control
    version INTEGER NOT NULL DEFAULT 1,
    parent_document_id UUID REFERENCES deal_documents(id), -- For document versioning

    -- Audit timestamps
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- STEP 2: Create indexes for efficient queries
-- ============================================================================

-- Primary lookup indexes
CREATE INDEX idx_deal_documents_deal_id ON deal_documents(deal_id)
    WHERE is_deleted = FALSE;

CREATE INDEX idx_deal_documents_uploaded_by ON deal_documents(uploaded_by);

CREATE INDEX idx_deal_documents_ingestion_status ON deal_documents(ingestion_status)
    WHERE is_deleted = FALSE;

CREATE INDEX idx_deal_documents_document_type ON deal_documents(document_type)
    WHERE is_deleted = FALSE;

CREATE INDEX idx_deal_documents_uploaded_at ON deal_documents(uploaded_at);

-- Composite index for deal + status queries
CREATE INDEX idx_deal_documents_deal_status ON deal_documents(deal_id, ingestion_status)
    WHERE is_deleted = FALSE;

-- Checksum index for duplicate detection
CREATE INDEX idx_deal_documents_checksum ON deal_documents(checksum)
    WHERE is_deleted = FALSE;

-- Parent document index for versioning
CREATE INDEX idx_deal_documents_parent ON deal_documents(parent_document_id)
    WHERE parent_document_id IS NOT NULL;

-- ============================================================================
-- STEP 3: Create document_ingestion_jobs table for async processing
-- ============================================================================

CREATE TABLE document_ingestion_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES deal_documents(id) ON DELETE CASCADE,

    -- Job status
    status VARCHAR(20) NOT NULL DEFAULT 'queued'
        CHECK (status IN ('queued', 'processing', 'completed', 'failed', 'cancelled')),

    -- Job timing
    queued_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    started_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,

    -- Processing details
    processing_attempts INTEGER DEFAULT 0,
    max_attempts INTEGER DEFAULT 3,
    error_message TEXT,

    -- Extracted data (JSON format for flexibility)
    extracted_metadata JSONB, -- Structured metadata extracted from document
    extracted_text TEXT, -- Full text content extracted from document

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- STEP 4: Create indexes for ingestion jobs
-- ============================================================================

CREATE INDEX idx_ingestion_jobs_document ON document_ingestion_jobs(document_id);
CREATE INDEX idx_ingestion_jobs_status ON document_ingestion_jobs(status);
CREATE INDEX idx_ingestion_jobs_queued_at ON document_ingestion_jobs(queued_at)
    WHERE status IN ('queued', 'processing');

-- ============================================================================
-- STEP 5: Create trigger to update timestamps
-- ============================================================================

CREATE OR REPLACE FUNCTION update_deal_documents_timestamp()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_deal_documents_timestamp
    BEFORE UPDATE ON deal_documents
    FOR EACH ROW
    EXECUTE FUNCTION update_deal_documents_timestamp();

CREATE TRIGGER trigger_update_ingestion_jobs_timestamp
    BEFORE UPDATE ON document_ingestion_jobs
    FOR EACH ROW
    EXECUTE FUNCTION update_deal_documents_timestamp();

-- ============================================================================
-- STEP 6: Create function to get document statistics per deal
-- ============================================================================

CREATE OR REPLACE FUNCTION get_deal_document_stats(p_deal_id UUID)
RETURNS TABLE(
    total_documents BIGINT,
    total_size_bytes BIGINT,
    pending_ingestion BIGINT,
    completed_ingestion BIGINT,
    failed_ingestion BIGINT,
    document_types JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        COUNT(*)::BIGINT as total_documents,
        COALESCE(SUM(file_size_bytes), 0)::BIGINT as total_size_bytes,
        COUNT(*) FILTER (WHERE ingestion_status = 'pending')::BIGINT as pending_ingestion,
        COUNT(*) FILTER (WHERE ingestion_status = 'completed')::BIGINT as completed_ingestion,
        COUNT(*) FILTER (WHERE ingestion_status = 'failed')::BIGINT as failed_ingestion,
        jsonb_object_agg(
            COALESCE(document_type, 'unknown'),
            count
        ) as document_types
    FROM deal_documents
    LEFT JOIN LATERAL (
        SELECT COUNT(*) as count
        FROM deal_documents d2
        WHERE d2.deal_id = p_deal_id
            AND d2.document_type = deal_documents.document_type
            AND d2.is_deleted = FALSE
    ) counts ON true
    WHERE deal_id = p_deal_id
        AND is_deleted = FALSE
    GROUP BY deal_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- STEP 7: Add comments for documentation
-- ============================================================================

COMMENT ON TABLE deal_documents IS
    'Stores metadata and references to documents associated with deals. Supports PDF, DOCX, XLSX, and other file types with versioning and ingestion tracking.';

COMMENT ON COLUMN deal_documents.storage_path IS
    'Storage location: S3 key for cloud storage or file system path for local storage';

COMMENT ON COLUMN deal_documents.ingestion_status IS
    'Tracks document processing status: pending (uploaded but not processed), processing (currently extracting content), completed (ready), failed (error occurred), skipped (intentionally not processed)';

COMMENT ON COLUMN deal_documents.checksum IS
    'SHA256 hash of file contents for integrity verification and duplicate detection';

COMMENT ON COLUMN deal_documents.version IS
    'Version number for document revisions. Links to parent_document_id for history tracking.';

COMMENT ON TABLE document_ingestion_jobs IS
    'Manages async document ingestion jobs for text extraction and metadata processing';

COMMENT ON COLUMN document_ingestion_jobs.extracted_metadata IS
    'Structured metadata in JSON format (e.g., author, creation date, properties)';

COMMENT ON COLUMN document_ingestion_jobs.extracted_text IS
    'Full text content extracted from document for search and analysis';

-- ============================================================================
-- Migration Complete
-- ============================================================================
