-- Create merchant turnover tracking and document management tables
-- Enables comprehensive business profile completion for credit line eligibility

-- ============================================================================
-- CREATE MERCHANT TURNOVER DAILY TABLE
-- ============================================================================

CREATE TABLE merchant_turnover_daily (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    gross_receipts DECIMAL(20,6) NOT NULL,
    refunds DECIMAL(20,6) DEFAULT 0,
    net_receipts DECIMAL(20,6) NOT NULL,
    source VARCHAR(20) DEFAULT 'MANUAL',
    verified BOOLEAN DEFAULT FALSE,
    verification_method VARCHAR(50),
    currency VARCHAR(3) DEFAULT 'USD',
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    UNIQUE(merchant_id, date)
);

-- Add indexes for performance
CREATE INDEX idx_merchant_turnover_merchant_id ON merchant_turnover_daily(merchant_id);
CREATE INDEX idx_merchant_turnover_date ON merchant_turnover_daily(date);
CREATE INDEX idx_merchant_turnover_verified ON merchant_turnover_daily(verified);
CREATE INDEX idx_merchant_turnover_merchant_date ON merchant_turnover_daily(merchant_id, date DESC);

-- ============================================================================
-- CREATE MERCHANT DOCUMENTS TABLE
-- ============================================================================

CREATE TABLE merchant_documents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    merchant_id UUID NOT NULL REFERENCES merchants(id) ON DELETE CASCADE,
    document_type VARCHAR(50) NOT NULL,
    document_url VARCHAR(500),
    document_hash VARCHAR(64),
    file_size_bytes INTEGER,
    original_filename VARCHAR(255),
    mime_type VARCHAR(100),
    uploaded_at TIMESTAMP DEFAULT NOW(),
    verified_at TIMESTAMP,
    status VARCHAR(20) DEFAULT 'PENDING',
    verification_notes TEXT,
    created_by UUID, -- Could reference identities table if needed
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Add indexes for performance
CREATE INDEX idx_merchant_documents_merchant_id ON merchant_documents(merchant_id);
CREATE INDEX idx_merchant_documents_type ON merchant_documents(document_type);
CREATE INDEX idx_merchant_documents_status ON merchant_documents(status);
CREATE INDEX idx_merchant_documents_uploaded ON merchant_documents(uploaded_at);

-- ============================================================================
-- ADD CONSTRAINTS
-- ============================================================================

-- Ensure valid sources for turnover data
ALTER TABLE merchant_turnover_daily ADD CONSTRAINT turnover_source_check 
    CHECK (source IN ('MANUAL', 'API', 'BANK_INTEGRATION'));

-- Ensure net_receipts calculation is correct
ALTER TABLE merchant_turnover_daily ADD CONSTRAINT turnover_net_receipts_check
    CHECK (net_receipts = gross_receipts - refunds);

-- Ensure positive amounts
ALTER TABLE merchant_turnover_daily ADD CONSTRAINT turnover_amounts_positive_check
    CHECK (gross_receipts >= 0 AND refunds >= 0 AND net_receipts >= 0);

-- Ensure valid document types
ALTER TABLE merchant_documents ADD CONSTRAINT documents_type_check 
    CHECK (document_type IN ('BANK_STATEMENT', 'TAX_RETURN', 'ARTICLES_OF_INCORPORATION', 'BUSINESS_LICENSE', 'FINANCIAL_STATEMENT', 'OTHER'));

-- Ensure valid document status
ALTER TABLE merchant_documents ADD CONSTRAINT documents_status_check
    CHECK (status IN ('PENDING', 'VERIFIED', 'REJECTED'));

-- ============================================================================
-- CREATE TRIGGER FOR UPDATED_AT TIMESTAMPS
-- ============================================================================

-- Use existing update_updated_at_column function

-- Triggers for turnover table
CREATE TRIGGER update_merchant_turnover_updated_at 
    BEFORE UPDATE ON merchant_turnover_daily 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Triggers for documents table  
CREATE TRIGGER update_merchant_documents_updated_at 
    BEFORE UPDATE ON merchant_documents 
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- CREATE VIEWS FOR ANALYTICS
-- ============================================================================

-- View for merchant turnover analytics
CREATE VIEW merchant_turnover_analytics AS
SELECT 
    m.id as merchant_id,
    m.legal_name,
    COUNT(t.id) as total_days_recorded,
    COUNT(CASE WHEN t.verified = true THEN 1 END) as verified_days,
    SUM(t.net_receipts) as total_net_receipts,
    AVG(t.net_receipts) as avg_daily_receipts,
    MAX(t.date) as last_turnover_date,
    MIN(t.date) as first_turnover_date,
    SUM(t.net_receipts) / NULLIF(COUNT(t.id), 0) * 30 as monthly_estimate,
    -- Calculate data quality score (0-100)
    CASE 
        WHEN COUNT(t.id) = 0 THEN 0
        ELSE (COUNT(CASE WHEN t.verified = true THEN 1 END)::float / COUNT(t.id) * 100)::integer
    END as data_quality_score
FROM merchants m
LEFT JOIN merchant_turnover_daily t ON m.id = t.merchant_id
GROUP BY m.id, m.legal_name;

-- View for merchant profile completion status
CREATE VIEW merchant_profile_completion AS
SELECT 
    m.id as merchant_id,
    m.legal_name,
    i.kyc_status,
    -- Basic merchant info completion
    CASE WHEN m.legal_name IS NOT NULL AND LENGTH(m.legal_name) > 0 THEN true ELSE false END as has_legal_name,
    CASE WHEN m.tax_id IS NOT NULL AND LENGTH(m.tax_id) > 0 THEN true ELSE false END as has_tax_id,
    CASE WHEN m.mcc IS NOT NULL AND LENGTH(m.mcc) > 0 THEN true ELSE false END as has_mcc,
    
    -- Business profile completion  
    CASE WHEN mp.industry IS NOT NULL AND LENGTH(mp.industry) > 0 THEN true ELSE false END as has_industry,
    CASE WHEN mp.years_in_business IS NOT NULL AND mp.years_in_business > 0 THEN true ELSE false END as has_years_in_business,
    CASE WHEN mp.monthly_revenue_estimate IS NOT NULL AND mp.monthly_revenue_estimate > 0 THEN true ELSE false END as has_revenue_estimate,
    
    -- KYC status
    CASE WHEN i.kyc_status = 'APPROVED' THEN true ELSE false END as kyc_approved,
    
    -- Turnover history (minimum 30 days for basic profile)
    COALESCE(ta.total_days_recorded, 0) >= 30 as has_sufficient_turnover_history,
    COALESCE(ta.verified_days, 0) >= 15 as has_verified_turnover_data,
    
    -- Document requirements
    COUNT(CASE WHEN md.document_type = 'BANK_STATEMENT' AND md.status = 'VERIFIED' THEN 1 END) >= 1 as has_bank_statement,
    
    -- Overall completion score (0-100)
    (
        (CASE WHEN m.legal_name IS NOT NULL THEN 10 ELSE 0 END) +
        (CASE WHEN m.tax_id IS NOT NULL THEN 10 ELSE 0 END) +
        (CASE WHEN m.mcc IS NOT NULL THEN 10 ELSE 0 END) +
        (CASE WHEN mp.industry IS NOT NULL THEN 10 ELSE 0 END) +
        (CASE WHEN mp.years_in_business IS NOT NULL THEN 10 ELSE 0 END) +
        (CASE WHEN mp.monthly_revenue_estimate IS NOT NULL THEN 10 ELSE 0 END) +
        (CASE WHEN i.kyc_status = 'APPROVED' THEN 20 ELSE 0 END) +
        (CASE WHEN COALESCE(ta.total_days_recorded, 0) >= 30 THEN 20 ELSE 0 END)
    ) as completion_score
    
FROM merchants m
LEFT JOIN identities i ON m.user_id = i.id
LEFT JOIN merchant_profiles mp ON m.id = mp.merchant_id
LEFT JOIN merchant_turnover_analytics ta ON m.id = ta.merchant_id
LEFT JOIN merchant_documents md ON m.id = md.merchant_id
GROUP BY m.id, m.legal_name, i.kyc_status, mp.industry, mp.years_in_business, 
         mp.monthly_revenue_estimate, ta.total_days_recorded, ta.verified_days;

-- ============================================================================
-- SAMPLE DATA FOR TESTING (COMMENTED OUT)
-- ============================================================================

-- Test data for development (uncomment for testing)
/*
-- Insert sample turnover data
INSERT INTO merchant_turnover_daily (merchant_id, date, gross_receipts, refunds, net_receipts, verified) 
VALUES 
    ((SELECT id FROM merchants LIMIT 1), CURRENT_DATE - INTERVAL '1 day', 1500.00, 50.00, 1450.00, true),
    ((SELECT id FROM merchants LIMIT 1), CURRENT_DATE - INTERVAL '2 days', 2200.00, 100.00, 2100.00, true),
    ((SELECT id FROM merchants LIMIT 1), CURRENT_DATE - INTERVAL '3 days', 1800.00, 75.00, 1725.00, false);

-- Insert sample document
INSERT INTO merchant_documents (merchant_id, document_type, document_url, status)
VALUES ((SELECT id FROM merchants LIMIT 1), 'BANK_STATEMENT', '/uploads/bank_statement_202501.pdf', 'PENDING');
*/