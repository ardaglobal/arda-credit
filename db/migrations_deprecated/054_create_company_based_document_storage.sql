-- Company-Based Document Storage Infrastructure
-- This migration adds company-based document storage with role-based access controls
-- and ZK proof integration for the Arda Credit ecosystem

-- ============================================================================
-- COMPANY MANAGEMENT TABLES
-- ============================================================================

-- Table for company/organization management
CREATE TABLE companies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    company_type VARCHAR(20) NOT NULL CHECK (company_type IN ('originator', 'investor')),
    kyb_status VARCHAR(20) NOT NULL DEFAULT 'not_submitted' CHECK (kyb_status IN ('not_submitted', 'pending', 'approved', 'rejected')),
    kyb_submitted_at TIMESTAMP WITH TIME ZONE,
    kyb_verified_at TIMESTAMP WITH TIME ZONE,
    
    -- Company contact and legal information
    legal_name VARCHAR(500),
    tax_id VARCHAR(50),
    registration_number VARCHAR(100),
    jurisdiction VARCHAR(100),
    address_line1 VARCHAR(255),
    address_line2 VARCHAR(255),
    city VARCHAR(100),
    state_province VARCHAR(100),
    postal_code VARCHAR(20),
    country VARCHAR(100),
    
    -- Document storage configuration
    s3_bucket_prefix VARCHAR(100), -- Company-specific S3 bucket prefix
    encryption_key_id VARCHAR(255), -- AWS KMS key ID for company-specific encryption
    document_retention_days INTEGER DEFAULT 2555, -- ~7 years default retention
    
    -- Audit and compliance
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID REFERENCES identities(id),
    
    UNIQUE(name, company_type) -- Prevent duplicate company names within same type
);

-- Table for team member roles within companies
CREATE TABLE team_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('admin', 'write', 'read')),
    
    -- Role-specific permissions
    can_upload_documents BOOLEAN DEFAULT FALSE,
    can_download_documents BOOLEAN DEFAULT TRUE,
    can_delete_documents BOOLEAN DEFAULT FALSE,
    can_approve_documents BOOLEAN DEFAULT FALSE,
    can_manage_team BOOLEAN DEFAULT FALSE,
    can_view_audit_logs BOOLEAN DEFAULT FALSE,
    
    -- Status and lifecycle
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive', 'suspended')),
    invited_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    joined_at TIMESTAMP WITH TIME ZONE,
    last_active_at TIMESTAMP WITH TIME ZONE,
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID REFERENCES identities(id),
    
    UNIQUE(company_id, user_id) -- One role per user per company
);

-- ============================================================================
-- DOCUMENT STORAGE TABLES
-- ============================================================================

-- Table for document metadata and access control
CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    
    -- Document identification
    filename VARCHAR(500) NOT NULL,
    original_filename VARCHAR(500) NOT NULL,
    file_extension VARCHAR(10) NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    file_size_bytes BIGINT NOT NULL CHECK (file_size_bytes > 0),
    
    -- Document categorization
    document_type VARCHAR(50) NOT NULL CHECK (document_type IN ('loan_contract', 'financial_statement', 'kyc_document', 'legal_document', 'other')),
    deal_id UUID, -- Optional reference to specific deal
    category VARCHAR(100), -- Additional categorization
    tags TEXT[], -- Array of tags for searchability
    
    -- Storage and security
    s3_bucket VARCHAR(100) NOT NULL,
    s3_key VARCHAR(500) NOT NULL,
    s3_version_id VARCHAR(100), -- S3 object version for immutability
    
    -- Document integrity and verification
    sha256_hash VARCHAR(64) NOT NULL, -- SHA-256 hash for integrity verification
    document_fingerprint VARCHAR(128), -- Additional fingerprint for duplicate detection
    encryption_status VARCHAR(20) DEFAULT 'encrypted' CHECK (encryption_status IN ('encrypted', 'unencrypted')),
    
    -- Access control and workflow
    access_level VARCHAR(20) NOT NULL DEFAULT 'company' CHECK (access_level IN ('public', 'company', 'deal', 'restricted')),
    approval_status VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (approval_status IN ('pending', 'approved', 'rejected')),
    approved_by UUID REFERENCES identities(id),
    approved_at TIMESTAMP WITH TIME ZONE,
    
    -- Document lifecycle
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived', 'deleted')),
    expires_at TIMESTAMP WITH TIME ZONE, -- Optional expiration date
    archived_at TIMESTAMP WITH TIME ZONE,
    deleted_at TIMESTAMP WITH TIME ZONE,
    
    -- Metadata and versioning
    version INTEGER DEFAULT 1,
    parent_document_id UUID REFERENCES documents(id), -- For document revisions
    metadata JSONB, -- Additional document metadata
    
    -- Audit trail
    uploaded_by UUID NOT NULL REFERENCES identities(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    
    UNIQUE(s3_bucket, s3_key), -- Ensure unique S3 storage location
    UNIQUE(sha256_hash, company_id) -- Prevent duplicate documents within company
);

-- Table for document access audit logs
CREATE TABLE document_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id),
    
    -- Audit event details
    action VARCHAR(50) NOT NULL CHECK (action IN ('upload', 'download', 'view', 'delete', 'approve', 'reject', 'share', 'update_metadata')),
    result VARCHAR(20) NOT NULL CHECK (result IN ('success', 'failure', 'denied')),
    
    -- Request context
    ip_address INET,
    user_agent TEXT,
    request_id VARCHAR(100), -- For correlating with application logs
    
    -- Additional context
    details JSONB, -- Additional audit details (file size, permissions checked, etc.)
    error_message TEXT, -- If result = 'failure' or 'denied'
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for document sharing and temporary access
CREATE TABLE document_shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    
    -- Sharing configuration
    share_token VARCHAR(128) NOT NULL UNIQUE, -- Secure random token for access
    share_type VARCHAR(20) NOT NULL CHECK (share_type IN ('download', 'view', 'temporary')),
    
    -- Access control
    allowed_downloads INTEGER DEFAULT 1, -- Number of allowed downloads
    downloads_used INTEGER DEFAULT 0,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    
    -- Sharing context
    shared_by UUID NOT NULL REFERENCES identities(id),
    shared_with_email VARCHAR(255), -- Optional: specific email restriction
    access_restrictions JSONB, -- Additional access restrictions
    
    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'expired', 'revoked')),
    revoked_at TIMESTAMP WITH TIME ZONE,
    revoked_by UUID REFERENCES identities(id),
    
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- ZK PROOF INTEGRATION TABLES
-- ============================================================================

-- Table for document state transitions in ZK proofs
CREATE TABLE document_state_transitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    
    -- ZK proof integration
    transaction_hash BYTEA NOT NULL, -- Hash for zkVM compatibility
    state_transition_type VARCHAR(50) NOT NULL CHECK (state_transition_type IN ('document_upload', 'document_approval', 'document_deletion', 'access_grant', 'access_revoke')),
    
    -- State data
    previous_state_hash BYTEA,
    new_state_hash BYTEA NOT NULL,
    merkle_proof BYTEA, -- Merkle proof for state inclusion
    
    -- Batch processing integration
    included_in_batch_id INTEGER REFERENCES proof_batches(id),
    batch_position INTEGER, -- Position within the batch
    
    -- Metadata
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_by UUID NOT NULL REFERENCES identities(id)
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Company indexes
CREATE INDEX idx_companies_company_type ON companies(company_type);
CREATE INDEX idx_companies_kyb_status ON companies(kyb_status);
CREATE INDEX idx_companies_created_at ON companies(created_at);
CREATE INDEX idx_companies_name_type ON companies(name, company_type);

-- Team member indexes
CREATE INDEX idx_team_members_company_id ON team_members(company_id);
CREATE INDEX idx_team_members_user_id ON team_members(user_id);
CREATE INDEX idx_team_members_role ON team_members(role);
CREATE INDEX idx_team_members_status ON team_members(status);
CREATE INDEX idx_team_members_company_role ON team_members(company_id, role);

-- Document indexes
CREATE INDEX idx_documents_company_id ON documents(company_id);
CREATE INDEX idx_documents_document_type ON documents(document_type);
CREATE INDEX idx_documents_status ON documents(status);
CREATE INDEX idx_documents_uploaded_by ON documents(uploaded_by);
CREATE INDEX idx_documents_created_at ON documents(created_at);
CREATE INDEX idx_documents_sha256_hash ON documents(sha256_hash);
CREATE INDEX idx_documents_deal_id ON documents(deal_id) WHERE deal_id IS NOT NULL;
CREATE INDEX idx_documents_approval_status ON documents(approval_status);
CREATE INDEX idx_documents_company_type_status ON documents(company_id, document_type, status);

-- Audit log indexes
CREATE INDEX idx_document_audit_logs_document_id ON document_audit_logs(document_id);
CREATE INDEX idx_document_audit_logs_company_id ON document_audit_logs(company_id);
CREATE INDEX idx_document_audit_logs_user_id ON document_audit_logs(user_id);
CREATE INDEX idx_document_audit_logs_action ON document_audit_logs(action);
CREATE INDEX idx_document_audit_logs_created_at ON document_audit_logs(created_at);
CREATE INDEX idx_document_audit_logs_company_action_date ON document_audit_logs(company_id, action, created_at);

-- Document share indexes
CREATE INDEX idx_document_shares_document_id ON document_shares(document_id);
CREATE INDEX idx_document_shares_share_token ON document_shares(share_token);
CREATE INDEX idx_document_shares_expires_at ON document_shares(expires_at);
CREATE INDEX idx_document_shares_status ON document_shares(status);
CREATE INDEX idx_document_shares_shared_by ON document_shares(shared_by);

-- ZK integration indexes
CREATE INDEX idx_document_state_transitions_document_id ON document_state_transitions(document_id);
CREATE INDEX idx_document_state_transitions_company_id ON document_state_transitions(company_id);
CREATE INDEX idx_document_state_transitions_batch_id ON document_state_transitions(included_in_batch_id) WHERE included_in_batch_id IS NOT NULL;
CREATE INDEX idx_document_state_transitions_type ON document_state_transitions(state_transition_type);
CREATE INDEX idx_document_state_transitions_created_at ON document_state_transitions(created_at);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Triggers for updated_at timestamps
CREATE TRIGGER update_companies_updated_at BEFORE UPDATE ON companies FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_team_members_updated_at BEFORE UPDATE ON team_members FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_documents_updated_at BEFORE UPDATE ON documents FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_document_shares_updated_at BEFORE UPDATE ON document_shares FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- ROLE-BASED PERMISSION FUNCTIONS
-- ============================================================================

-- Function to set role-based permissions automatically
CREATE OR REPLACE FUNCTION set_role_permissions()
RETURNS TRIGGER AS $$
BEGIN
    -- Set permissions based on role
    CASE NEW.role
        WHEN 'admin' THEN
            NEW.can_upload_documents := TRUE;
            NEW.can_download_documents := TRUE;
            NEW.can_delete_documents := TRUE;
            NEW.can_approve_documents := TRUE;
            NEW.can_manage_team := TRUE;
            NEW.can_view_audit_logs := TRUE;
        WHEN 'write' THEN
            NEW.can_upload_documents := TRUE;
            NEW.can_download_documents := TRUE;
            NEW.can_delete_documents := FALSE;
            NEW.can_approve_documents := TRUE;
            NEW.can_manage_team := FALSE;
            NEW.can_view_audit_logs := FALSE;
        WHEN 'read' THEN
            NEW.can_upload_documents := FALSE;
            NEW.can_download_documents := TRUE;
            NEW.can_delete_documents := FALSE;
            NEW.can_approve_documents := FALSE;
            NEW.can_manage_team := FALSE;
            NEW.can_view_audit_logs := FALSE;
    END CASE;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Trigger to automatically set permissions based on role
CREATE TRIGGER set_team_member_permissions
    BEFORE INSERT OR UPDATE ON team_members
    FOR EACH ROW
    EXECUTE FUNCTION set_role_permissions();

-- ============================================================================
-- DOCUMENT AUDIT LOGGING FUNCTIONS
-- ============================================================================

-- Function to automatically log document access
CREATE OR REPLACE FUNCTION log_document_access(
    p_document_id UUID,
    p_user_id UUID,
    p_action VARCHAR(50),
    p_result VARCHAR(20),
    p_ip_address INET DEFAULT NULL,
    p_user_agent TEXT DEFAULT NULL,
    p_request_id VARCHAR(100) DEFAULT NULL,
    p_details JSONB DEFAULT NULL,
    p_error_message TEXT DEFAULT NULL
)
RETURNS UUID AS $$
DECLARE
    audit_log_id UUID;
    doc_company_id UUID;
BEGIN
    -- Get company_id from document
    SELECT company_id INTO doc_company_id
    FROM documents
    WHERE id = p_document_id;
    
    -- Insert audit log entry
    INSERT INTO document_audit_logs (
        document_id,
        company_id,
        user_id,
        action,
        result,
        ip_address,
        user_agent,
        request_id,
        details,
        error_message
    ) VALUES (
        p_document_id,
        doc_company_id,
        p_user_id,
        p_action,
        p_result,
        p_ip_address,
        p_user_agent,
        p_request_id,
        p_details,
        p_error_message
    ) RETURNING id INTO audit_log_id;
    
    RETURN audit_log_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- DOCUMENT PERMISSION CHECKING FUNCTIONS
-- ============================================================================

-- Function to check if user has permission for document operation
CREATE OR REPLACE FUNCTION check_document_permission(
    p_user_id UUID,
    p_document_id UUID,
    p_action VARCHAR(50)
)
RETURNS BOOLEAN AS $$
DECLARE
    user_role VARCHAR(20);
    user_permissions RECORD;
    doc_company_id UUID;
    user_kyc_approved BOOLEAN;
    company_kyb_approved BOOLEAN;
BEGIN
    -- Get document's company
    SELECT company_id INTO doc_company_id
    FROM documents
    WHERE id = p_document_id;
    
    IF doc_company_id IS NULL THEN
        RETURN FALSE; -- Document not found
    END IF;
    
    -- Get user's role and permissions in the company
    SELECT 
        tm.role,
        tm.can_upload_documents,
        tm.can_download_documents,
        tm.can_delete_documents,
        tm.can_approve_documents,
        tm.can_view_audit_logs
    INTO user_permissions
    FROM team_members tm
    WHERE tm.company_id = doc_company_id 
    AND tm.user_id = p_user_id 
    AND tm.status = 'active';
    
    IF NOT FOUND THEN
        RETURN FALSE; -- User not a member of the company
    END IF;
    
    -- Check KYC status for user
    SELECT (kyc_status = 'approved') INTO user_kyc_approved
    FROM identities
    WHERE id = p_user_id;
    
    -- Check KYB status for company
    SELECT (kyb_status = 'approved') INTO company_kyb_approved
    FROM companies
    WHERE id = doc_company_id;
    
    -- For sensitive operations, require both KYC and KYB approval
    IF p_action IN ('upload', 'delete', 'approve') AND 
       (NOT user_kyc_approved OR NOT company_kyb_approved) THEN
        RETURN FALSE;
    END IF;
    
    -- Check specific permissions based on action
    CASE p_action
        WHEN 'upload' THEN
            RETURN user_permissions.can_upload_documents;
        WHEN 'download', 'view' THEN
            RETURN user_permissions.can_download_documents;
        WHEN 'delete' THEN
            RETURN user_permissions.can_delete_documents;
        WHEN 'approve', 'reject' THEN
            RETURN user_permissions.can_approve_documents;
        WHEN 'view_audit' THEN
            RETURN user_permissions.can_view_audit_logs;
        ELSE
            RETURN FALSE; -- Unknown action
    END CASE;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- DOCUMENT CLEANUP AND MAINTENANCE FUNCTIONS
-- ============================================================================

-- Function to clean up expired document shares
CREATE OR REPLACE FUNCTION cleanup_expired_document_shares()
RETURNS INTEGER AS $$
DECLARE
    cleaned_count INTEGER := 0;
BEGIN
    -- Mark expired shares as expired
    UPDATE document_shares 
    SET 
        status = 'expired',
        updated_at = NOW()
    WHERE expires_at < NOW()
    AND status = 'active';
    
    GET DIAGNOSTICS cleaned_count = ROW_COUNT;
    
    RETURN cleaned_count;
END;
$$ LANGUAGE plpgsql;

-- Function to archive old documents based on company retention policies
CREATE OR REPLACE FUNCTION archive_old_documents()
RETURNS INTEGER AS $$
DECLARE
    archived_count INTEGER := 0;
BEGIN
    -- Archive documents past their company's retention period
    UPDATE documents 
    SET 
        status = 'archived',
        archived_at = NOW(),
        updated_at = NOW()
    FROM companies c
    WHERE documents.company_id = c.id
    AND documents.status = 'active'
    AND documents.created_at < (NOW() - INTERVAL '1 day' * c.document_retention_days);
    
    GET DIAGNOSTICS archived_count = ROW_COUNT;
    
    RETURN archived_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE companies IS 'Company/organization management with KYB compliance and document storage configuration';
COMMENT ON TABLE team_members IS 'Team member roles within companies with granular permissions for document operations';
COMMENT ON TABLE documents IS 'Document metadata and access control with S3 storage references and integrity verification';
COMMENT ON TABLE document_audit_logs IS 'Comprehensive audit trail for all document operations and access attempts';
COMMENT ON TABLE document_shares IS 'Temporary document sharing with secure tokens and access controls';
COMMENT ON TABLE document_state_transitions IS 'ZK proof integration for document operations and state transitions';

COMMENT ON COLUMN companies.kyb_status IS 'Know Your Business verification status: not_submitted, pending, approved, rejected';
COMMENT ON COLUMN companies.s3_bucket_prefix IS 'Company-specific S3 bucket prefix for document organization';
COMMENT ON COLUMN companies.encryption_key_id IS 'AWS KMS key ID for company-specific document encryption';
COMMENT ON COLUMN team_members.role IS 'Team member role: admin (full access), write (upload/approve), read (view only)';
COMMENT ON COLUMN documents.sha256_hash IS 'SHA-256 hash for document integrity verification and ZK proof integration';
COMMENT ON COLUMN documents.document_fingerprint IS 'Additional fingerprint for duplicate detection across deals';
COMMENT ON COLUMN documents.access_level IS 'Document access level: public, company, deal, restricted';
COMMENT ON COLUMN document_audit_logs.action IS 'Audit action: upload, download, view, delete, approve, reject, share, update_metadata';
COMMENT ON COLUMN document_state_transitions.state_transition_type IS 'ZK state transition type for document operations';