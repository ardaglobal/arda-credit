-- ============================================================================
-- INVITE CODES AND ONBOARDING
-- User invitation system with type-specific codes
-- ============================================================================

-- ============================================================================
-- INVITE CODES TABLE
-- ============================================================================

-- Table for invite codes that can be used during user onboarding
CREATE TABLE invite_codes (
    id SERIAL PRIMARY KEY,
    code VARCHAR(64) NOT NULL UNIQUE,
    user_type VARCHAR(20) NOT NULL DEFAULT 'investor' CHECK (user_type IN ('investor', 'originator', 'admin')),
    created_by UUID REFERENCES users(id), -- Optional: who created this invite code
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP WITH TIME ZONE, -- Optional: expiration date
    is_active BOOLEAN DEFAULT TRUE,
    used_by UUID REFERENCES users(id), -- Track who used the code
    used_at TIMESTAMP WITH TIME ZONE, -- When it was used
    max_uses INTEGER DEFAULT 1, -- How many times this code can be used
    current_uses INTEGER DEFAULT 0, -- How many times it has been used
    description TEXT, -- Optional: description of what this invite is for

    -- Constraints
    CHECK (current_uses >= 0),
    CHECK (max_uses >= 1),
    CHECK (current_uses <= max_uses)
);

-- ============================================================================
-- DOCUMENT STORAGE TABLES
-- ============================================================================

-- Document metadata and access control (from company-based document storage)
CREATE TABLE documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id UUID REFERENCES companies(id) ON DELETE CASCADE,

    -- Document identification
    filename VARCHAR(500) NOT NULL,
    original_filename VARCHAR(500) NOT NULL,
    file_extension VARCHAR(10) NOT NULL,
    mime_type VARCHAR(100) NOT NULL,
    file_size_bytes BIGINT NOT NULL CHECK (file_size_bytes > 0),

    -- Document categorization
    document_type VARCHAR(50) NOT NULL CHECK (document_type IN ('loan_contract', 'financial_statement', 'kyc_document', 'legal_document', 'other')),
    deal_id UUID REFERENCES deals(id), -- Optional reference to specific deal
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
    approved_by UUID REFERENCES users(id),
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
    uploaded_by UUID NOT NULL REFERENCES users(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(s3_bucket, s3_key), -- Ensure unique S3 storage location
    UNIQUE(sha256_hash, company_id) -- Prevent duplicate documents within company
);

-- Document access audit logs
CREATE TABLE document_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    company_id UUID NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id),

    -- Audit event details
    action VARCHAR(50) NOT NULL CHECK (action IN ('upload', 'download', 'view', 'delete', 'approve', 'reject', 'share', 'update_metadata')),
    result VARCHAR(20) NOT NULL CHECK (result IN ('success', 'failure', 'denied')),

    -- Request context
    ip_address INET,
    user_agent TEXT,
    request_id VARCHAR(100), -- For correlating with application logs

    -- Additional context
    details JSONB, -- Additional audit details
    error_message TEXT, -- If result = 'failure' or 'denied'

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Document sharing and temporary access
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
    shared_by UUID NOT NULL REFERENCES users(id),
    shared_with_email VARCHAR(255), -- Optional: specific email restriction
    access_restrictions JSONB, -- Additional access restrictions

    -- Status
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'expired', 'revoked')),
    revoked_at TIMESTAMP WITH TIME ZONE,
    revoked_by UUID REFERENCES users(id),

    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- AI ANALYSIS TABLES
-- ============================================================================

-- AI document analysis results
CREATE TABLE ai_document_analysis (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    analysis_type VARCHAR(50) NOT NULL, -- 'risk_assessment', 'content_extraction', 'compliance_check'
    ai_provider VARCHAR(50) NOT NULL, -- 'openai', 'anthropic', 'azure_openai', etc.
    model_used VARCHAR(100) NOT NULL, -- e.g., 'gpt-4', 'claude-3-opus'

    -- Analysis results
    analysis_result JSONB NOT NULL, -- Structured analysis results
    confidence_score DECIMAL(3, 2), -- 0.00 to 1.00
    processing_time_ms INTEGER,

    -- Request context
    prompt_template TEXT, -- The prompt template used
    tokens_used INTEGER, -- For cost tracking
    cost_estimate DECIMAL(10, 4), -- Estimated cost in USD

    -- Status and lifecycle
    status VARCHAR(20) NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'completed', 'failed')),
    error_message TEXT, -- If status = 'failed'
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    -- Audit trail
    requested_by UUID NOT NULL REFERENCES users(id)
);

-- AI chat conversations
CREATE TABLE ai_chat_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title VARCHAR(255),
    conversation_context JSONB, -- Context like document_ids, deal_ids, etc.
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Individual chat messages within conversations
CREATE TABLE ai_chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES ai_chat_conversations(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    tokens_used INTEGER,
    model_used VARCHAR(100),
    cost_estimate DECIMAL(10, 4),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Invite code indexes
CREATE INDEX idx_invite_codes_code ON invite_codes(code);
CREATE INDEX idx_invite_codes_active_unexpired ON invite_codes(is_active, expires_at) WHERE is_active = TRUE;
CREATE INDEX idx_invite_codes_usage ON invite_codes(current_uses, max_uses);
CREATE INDEX idx_invite_codes_created_by ON invite_codes(created_by) WHERE created_by IS NOT NULL;
CREATE INDEX idx_invite_codes_user_type ON invite_codes(user_type);

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

-- Document audit log indexes
CREATE INDEX idx_document_audit_logs_document_id ON document_audit_logs(document_id);
CREATE INDEX idx_document_audit_logs_company_id ON document_audit_logs(company_id);
CREATE INDEX idx_document_audit_logs_user_id ON document_audit_logs(user_id);
CREATE INDEX idx_document_audit_logs_action ON document_audit_logs(action);
CREATE INDEX idx_document_audit_logs_created_at ON document_audit_logs(created_at);

-- Document share indexes
CREATE INDEX idx_document_shares_document_id ON document_shares(document_id);
CREATE INDEX idx_document_shares_share_token ON document_shares(share_token);
CREATE INDEX idx_document_shares_expires_at ON document_shares(expires_at);
CREATE INDEX idx_document_shares_status ON document_shares(status);
CREATE INDEX idx_document_shares_shared_by ON document_shares(shared_by);

-- AI analysis indexes
CREATE INDEX idx_ai_document_analysis_document_id ON ai_document_analysis(document_id);
CREATE INDEX idx_ai_document_analysis_type ON ai_document_analysis(analysis_type);
CREATE INDEX idx_ai_document_analysis_status ON ai_document_analysis(status);
CREATE INDEX idx_ai_document_analysis_requested_by ON ai_document_analysis(requested_by);

-- AI chat indexes
CREATE INDEX idx_ai_chat_conversations_user_id ON ai_chat_conversations(user_id);
CREATE INDEX idx_ai_chat_messages_conversation_id ON ai_chat_messages(conversation_id);
CREATE INDEX idx_ai_chat_messages_created_at ON ai_chat_messages(created_at);

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to check if invite code is valid and available
CREATE OR REPLACE FUNCTION is_invite_code_valid(invite_code VARCHAR(64))
RETURNS BOOLEAN AS $$
DECLARE
    code_record RECORD;
BEGIN
    -- Get the invite code record
    SELECT * INTO code_record
    FROM invite_codes
    WHERE code = invite_code;

    -- Return false if code doesn't exist
    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;

    -- Check if code is active
    IF NOT code_record.is_active THEN
        RETURN FALSE;
    END IF;

    -- Check if code has expired
    IF code_record.expires_at IS NOT NULL AND code_record.expires_at < CURRENT_TIMESTAMP THEN
        RETURN FALSE;
    END IF;

    -- Check if code has reached max uses
    IF code_record.current_uses >= code_record.max_uses THEN
        RETURN FALSE;
    END IF;

    -- Code is valid
    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Function to use an invite code (marks it as used and increments counter)
CREATE OR REPLACE FUNCTION use_invite_code(invite_code VARCHAR(64), user_id UUID DEFAULT NULL)
RETURNS BOOLEAN AS $$
DECLARE
    code_record RECORD;
BEGIN
    -- First check if code is valid
    IF NOT is_invite_code_valid(invite_code) THEN
        RETURN FALSE;
    END IF;

    -- Update the code usage
    UPDATE invite_codes
    SET
        current_uses = current_uses + 1,
        used_by = COALESCE(user_id, used_by), -- Only set if provided and not already set
        used_at = CASE
            WHEN current_uses = 0 THEN CURRENT_TIMESTAMP
            ELSE used_at
        END
    WHERE code = invite_code;

    RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Function to create a new invite code
CREATE OR REPLACE FUNCTION create_invite_code(
    invite_code VARCHAR(64),
    creator_id UUID DEFAULT NULL,
    expiry_date TIMESTAMP WITH TIME ZONE DEFAULT NULL,
    max_usage INTEGER DEFAULT 1,
    code_description TEXT DEFAULT NULL,
    invite_user_type VARCHAR(20) DEFAULT 'investor'
)
RETURNS BOOLEAN AS $$
BEGIN
    -- Validate user_type
    IF invite_user_type NOT IN ('investor', 'originator', 'admin') THEN
        RAISE EXCEPTION 'Invalid user_type: %. Must be one of: investor, originator, admin', invite_user_type;
    END IF;

    INSERT INTO invite_codes (
        code,
        created_by,
        expires_at,
        max_uses,
        description,
        user_type
    ) VALUES (
        invite_code,
        creator_id,
        expiry_date,
        max_usage,
        code_description,
        invite_user_type
    );

    RETURN TRUE;
EXCEPTION
    WHEN unique_violation THEN
        -- Code already exists
        RETURN FALSE;
END;
$$ LANGUAGE plpgsql;

-- Trigger for updated_at timestamps
CREATE TRIGGER update_documents_updated_at BEFORE UPDATE ON documents FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_document_shares_updated_at BEFORE UPDATE ON document_shares FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_ai_chat_conversations_updated_at BEFORE UPDATE ON ai_chat_conversations FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- INSERT DEFAULT INVITE CODES
-- ============================================================================

-- Insert the three 2025 invite codes
INSERT INTO invite_codes (
    code,
    description,
    max_uses,
    expires_at,
    user_type,
    is_active
) VALUES
    (
        'INVESTOR2025',
        'Investor access invite code - expires December 31st, 2025',
        1000,
        '2025-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'investor',
        true
    ),
    (
        'ORIGINATOR2025',
        'Originator access invite code - expires December 31st, 2025',
        1000,
        '2025-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'originator',
        true
    ),
    (
        'ADMIN2025',
        'Admin access invite code - expires December 31st, 2025',
        1000,
        '2025-12-31 23:59:59'::TIMESTAMP WITH TIME ZONE,
        'admin',
        true
    )
ON CONFLICT (code) DO NOTHING;

-- Insert a test engineering user for development
INSERT INTO users (
    id,
    user_type,
    email,
    first_name,
    last_name,
    kyc_status,
    kyc_verified_at,
    email_verified,
    system_admin,
    created_at
) VALUES (
    'f47ac10b-58cc-4372-a567-0e02b2c3d479'::UUID,
    'admin',
    'engineering-test@arda.xyz',
    'Engineering',
    'Test',
    'approved',
    NOW(),
    true,
    true,
    NOW()
)
ON CONFLICT (email) DO NOTHING;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE invite_codes IS 'Invite codes for user registration with type-specific access control';
COMMENT ON TABLE documents IS 'Document metadata and access control with S3 storage references and integrity verification';
COMMENT ON TABLE document_audit_logs IS 'Comprehensive audit trail for all document operations and access attempts';
COMMENT ON TABLE document_shares IS 'Temporary document sharing with secure tokens and access controls';
COMMENT ON TABLE ai_document_analysis IS 'AI-powered document analysis results for risk assessment and content extraction';
COMMENT ON TABLE ai_chat_conversations IS 'AI chat conversation sessions for user assistance';
COMMENT ON TABLE ai_chat_messages IS 'Individual messages within AI chat conversations';

COMMENT ON COLUMN invite_codes.user_type IS 'User type that this invite code grants access to: investor, originator, admin';
COMMENT ON COLUMN documents.sha256_hash IS 'SHA-256 hash for document integrity verification and ZK proof integration';
COMMENT ON COLUMN documents.document_fingerprint IS 'Additional fingerprint for duplicate detection across deals';
COMMENT ON COLUMN documents.access_level IS 'Document access level: public, company, deal, restricted';
COMMENT ON COLUMN document_audit_logs.action IS 'Audit action: upload, download, view, delete, approve, reject, share, update_metadata';
COMMENT ON COLUMN ai_document_analysis.confidence_score IS 'AI analysis confidence score from 0.00 to 1.00';
COMMENT ON COLUMN ai_chat_messages.role IS 'Message role: user (human), assistant (AI), system (automated)';