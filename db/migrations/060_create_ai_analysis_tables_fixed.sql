-- AI Document Analysis and Chat System (Fixed Version)
-- This migration creates tables for AI-powered document analysis that work with existing schema

-- Drop any partially created tables from previous attempt
DROP TABLE IF EXISTS ai_analysis_notifications CASCADE;
DROP TABLE IF EXISTS ai_analysis_triggers CASCADE;
DROP TABLE IF EXISTS ai_feedback CASCADE;
DROP TABLE IF EXISTS ai_negotiation_insights CASCADE;
DROP TABLE IF EXISTS ai_chat_messages CASCADE;
DROP TABLE IF EXISTS ai_chat_sessions CASCADE;
DROP TABLE IF EXISTS ai_change_analyses CASCADE;
DROP TABLE IF EXISTS ai_document_analyses CASCADE;

-- ============================================================================
-- AI DOCUMENT ANALYSIS TABLES
-- ============================================================================

-- Table for storing AI document analysis sessions
CREATE TABLE ai_document_analyses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    -- Remove references to document_versions for now
    analysis_type VARCHAR(50) NOT NULL DEFAULT 'full_diff',
    total_changes INTEGER NOT NULL DEFAULT 0,
    critical_changes_count INTEGER NOT NULL DEFAULT 0,
    overall_risk_score DECIMAL(3,2) DEFAULT 0.0 CHECK (overall_risk_score >= 0.0 AND overall_risk_score <= 1.0),
    summary TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'completed' CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    processing_time_ms INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for individual change analyses within a document analysis
CREATE TABLE ai_change_analyses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    analysis_id UUID NOT NULL REFERENCES ai_document_analyses(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    change_type VARCHAR(20) NOT NULL CHECK (change_type IN ('addition', 'deletion', 'modification', 'restructure')),
    impact_level VARCHAR(20) NOT NULL CHECK (impact_level IN ('minor', 'moderate', 'significant', 'critical')),
    category VARCHAR(20) NOT NULL CHECK (category IN ('financial', 'legal', 'operational', 'compliance')),
    summary TEXT NOT NULL,
    implications JSONB DEFAULT '[]'::jsonb,
    recommendations JSONB DEFAULT '[]'::jsonb,
    related_clauses JSONB DEFAULT '[]'::jsonb,
    risk_factors JSONB DEFAULT '[]'::jsonb,
    confidence_score DECIMAL(3,2) NOT NULL DEFAULT 0.0 CHECK (confidence_score >= 0.0 AND confidence_score <= 1.0),
    section_reference TEXT,
    line_numbers JSONB,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- AI CHAT AND CONVERSATION TABLES
-- ============================================================================

-- Table for AI chat sessions and conversations
CREATE TABLE ai_chat_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    document_id UUID REFERENCES documents(id) ON DELETE CASCADE,
    -- Remove reference to deals table for now
    session_name VARCHAR(255),
    session_type VARCHAR(50) NOT NULL DEFAULT 'document_analysis',
    context JSONB DEFAULT '{}'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_activity_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for individual chat messages
CREATE TABLE ai_chat_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES ai_chat_sessions(id) ON DELETE CASCADE,
    document_id UUID REFERENCES documents(id) ON DELETE SET NULL,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    context JSONB DEFAULT '{}'::jsonb,
    message_type VARCHAR(50) DEFAULT 'text',
    related_analysis_id UUID REFERENCES ai_document_analyses(id) ON DELETE SET NULL,
    confidence_score DECIMAL(3,2),
    tokens_used INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- AI INSIGHTS AND RECOMMENDATIONS TABLES
-- ============================================================================

-- Table for AI-generated negotiation insights
CREATE TABLE ai_negotiation_insights (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    analysis_id UUID REFERENCES ai_document_analyses(id) ON DELETE SET NULL,
    insight_type VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    market_comparison TEXT,
    historical_data JSONB,
    recommended_action TEXT,
    priority VARCHAR(20) NOT NULL CHECK (priority IN ('minor', 'moderate', 'significant', 'critical')),
    confidence_score DECIMAL(3,2) DEFAULT 0.0,
    tags JSONB DEFAULT '[]'::jsonb,
    status VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'addressed', 'dismissed')),
    created_by_user_id UUID REFERENCES identities(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for AI model performance and feedback
CREATE TABLE ai_feedback (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    analysis_id UUID REFERENCES ai_document_analyses(id) ON DELETE CASCADE,
    message_id UUID REFERENCES ai_chat_messages(id) ON DELETE CASCADE,
    insight_id UUID REFERENCES ai_negotiation_insights(id) ON DELETE CASCADE,
    feedback_type VARCHAR(50) NOT NULL,
    rating INTEGER CHECK (rating >= 1 AND rating <= 5),
    feedback_text TEXT,
    suggested_improvement TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- AI ANALYSIS TRIGGERS AND AUTOMATION
-- ============================================================================

-- Table for tracking automatic analysis triggers
CREATE TABLE ai_analysis_triggers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    trigger_type VARCHAR(50) NOT NULL,
    trigger_source VARCHAR(100),
    trigger_config JSONB DEFAULT '{}'::jsonb,
    is_active BOOLEAN NOT NULL DEFAULT true,
    last_triggered_at TIMESTAMP WITH TIME ZONE,
    next_trigger_at TIMESTAMP WITH TIME ZONE,
    created_by_user_id UUID REFERENCES identities(id) ON DELETE SET NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Table for AI analysis notifications and alerts
CREATE TABLE ai_analysis_notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    analysis_id UUID NOT NULL REFERENCES ai_document_analyses(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    notification_type VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    message TEXT NOT NULL,
    priority VARCHAR(20) NOT NULL CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
    delivery_method VARCHAR(20) DEFAULT 'in_app' CHECK (delivery_method IN ('in_app', 'email', 'sms', 'webhook')),
    is_read BOOLEAN NOT NULL DEFAULT false,
    read_at TIMESTAMP WITH TIME ZONE,
    sent_at TIMESTAMP WITH TIME ZONE,
    delivery_status VARCHAR(20) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- AI Document Analysis indexes
CREATE INDEX idx_ai_document_analyses_document_id ON ai_document_analyses(document_id);
CREATE INDEX idx_ai_document_analyses_created_at ON ai_document_analyses(created_at DESC);
CREATE INDEX idx_ai_document_analyses_status ON ai_document_analyses(status);
CREATE INDEX idx_ai_document_analyses_risk_score ON ai_document_analyses(overall_risk_score DESC);

-- AI Change Analysis indexes
CREATE INDEX idx_ai_change_analyses_analysis_id ON ai_change_analyses(analysis_id);
CREATE INDEX idx_ai_change_analyses_document_id ON ai_change_analyses(document_id);
CREATE INDEX idx_ai_change_analyses_impact_level ON ai_change_analyses(impact_level);
CREATE INDEX idx_ai_change_analyses_category ON ai_change_analyses(category);
CREATE INDEX idx_ai_change_analyses_confidence ON ai_change_analyses(confidence_score DESC);

-- AI Chat indexes
CREATE INDEX idx_ai_chat_sessions_user_id ON ai_chat_sessions(user_id);
CREATE INDEX idx_ai_chat_sessions_document_id ON ai_chat_sessions(document_id) WHERE document_id IS NOT NULL;
CREATE INDEX idx_ai_chat_sessions_active ON ai_chat_sessions(is_active, last_activity_at DESC);

CREATE INDEX idx_ai_chat_messages_session_id ON ai_chat_messages(session_id);
CREATE INDEX idx_ai_chat_messages_user_id ON ai_chat_messages(user_id);
CREATE INDEX idx_ai_chat_messages_created_at ON ai_chat_messages(created_at);
CREATE INDEX idx_ai_chat_messages_role ON ai_chat_messages(role);

-- AI Insights indexes
CREATE INDEX idx_ai_negotiation_insights_document_id ON ai_negotiation_insights(document_id);
CREATE INDEX idx_ai_negotiation_insights_priority ON ai_negotiation_insights(priority, status);
CREATE INDEX idx_ai_negotiation_insights_created_at ON ai_negotiation_insights(created_at DESC);
CREATE INDEX idx_ai_negotiation_insights_status ON ai_negotiation_insights(status);

-- AI Feedback indexes
CREATE INDEX idx_ai_feedback_user_id ON ai_feedback(user_id);
CREATE INDEX idx_ai_feedback_analysis_id ON ai_feedback(analysis_id) WHERE analysis_id IS NOT NULL;
CREATE INDEX idx_ai_feedback_rating ON ai_feedback(rating) WHERE rating IS NOT NULL;

-- AI Triggers and Notifications indexes
CREATE INDEX idx_ai_analysis_triggers_document_id ON ai_analysis_triggers(document_id);
CREATE INDEX idx_ai_analysis_triggers_active ON ai_analysis_triggers(is_active, next_trigger_at);

CREATE INDEX idx_ai_notifications_user_id ON ai_analysis_notifications(user_id);
CREATE INDEX idx_ai_notifications_analysis_id ON ai_analysis_notifications(analysis_id);
CREATE INDEX idx_ai_notifications_unread ON ai_analysis_notifications(user_id, is_read, created_at DESC);
CREATE INDEX idx_ai_notifications_priority ON ai_analysis_notifications(priority, created_at DESC);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update ai_document_analyses.updated_at on changes
CREATE TRIGGER update_ai_document_analyses_updated_at
    BEFORE UPDATE ON ai_document_analyses
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Update ai_negotiation_insights.updated_at on changes
CREATE TRIGGER update_ai_negotiation_insights_updated_at
    BEFORE UPDATE ON ai_negotiation_insights
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Update ai_analysis_triggers.updated_at on changes
CREATE TRIGGER update_ai_analysis_triggers_updated_at
    BEFORE UPDATE ON ai_analysis_triggers
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Update last_activity_at in chat sessions when messages are added
CREATE OR REPLACE FUNCTION update_chat_session_activity()
RETURNS TRIGGER AS $$
BEGIN
    UPDATE ai_chat_sessions
    SET last_activity_at = NEW.created_at
    WHERE id = NEW.session_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_chat_session_on_message
    AFTER INSERT ON ai_chat_messages
    FOR EACH ROW EXECUTE FUNCTION update_chat_session_activity();

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to get AI analysis statistics for a document
CREATE OR REPLACE FUNCTION get_document_ai_stats(doc_id UUID)
RETURNS TABLE(
    total_analyses INTEGER,
    average_risk_score DECIMAL(3,2),
    critical_changes_count INTEGER,
    last_analysis_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        COUNT(da.id)::INTEGER as total_analyses,
        AVG(da.overall_risk_score)::DECIMAL(3,2) as average_risk_score,
        SUM(da.critical_changes_count)::INTEGER as critical_changes_count,
        MAX(da.created_at) as last_analysis_at
    FROM ai_document_analyses da
    WHERE da.document_id = doc_id
    AND da.status = 'completed';
END;
$$ LANGUAGE plpgsql;

-- Function to get recent high-priority insights for a user
CREATE OR REPLACE FUNCTION get_user_priority_insights(
    user_id_param UUID,
    limit_count INTEGER DEFAULT 10
)
RETURNS TABLE(
    insight_id UUID,
    document_id UUID,
    title VARCHAR(255),
    priority VARCHAR(20),
    created_at TIMESTAMP WITH TIME ZONE
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        ni.id as insight_id,
        ni.document_id,
        ni.title,
        ni.priority,
        ni.created_at
    FROM ai_negotiation_insights ni
    JOIN documents d ON ni.document_id = d.id
    -- Join with document access or company access based on available tables
    WHERE ni.status = 'active'
    AND ni.priority IN ('significant', 'critical')
    ORDER BY
        CASE ni.priority
            WHEN 'critical' THEN 1
            WHEN 'significant' THEN 2
            WHEN 'moderate' THEN 3
            WHEN 'minor' THEN 4
        END,
        ni.created_at DESC
    LIMIT limit_count;
END;
$$ LANGUAGE plpgsql;

-- Function to clean up old chat sessions and messages
CREATE OR REPLACE FUNCTION cleanup_old_ai_chat_data(days_to_keep INTEGER DEFAULT 90)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    -- Mark inactive sessions older than specified days
    UPDATE ai_chat_sessions
    SET is_active = false
    WHERE last_activity_at < NOW() - INTERVAL '%d days' * days_to_keep
    AND is_active = true;

    -- Delete messages from inactive sessions older than retention period
    WITH deleted_messages AS (
        DELETE FROM ai_chat_messages
        WHERE session_id IN (
            SELECT id FROM ai_chat_sessions
            WHERE is_active = false
            AND last_activity_at < NOW() - INTERVAL '%d days' * (days_to_keep + 30)
        )
        RETURNING id
    )
    SELECT COUNT(*) INTO deleted_count FROM deleted_messages;

    -- Delete old inactive sessions
    DELETE FROM ai_chat_sessions
    WHERE is_active = false
    AND last_activity_at < NOW() - INTERVAL '%d days' * (days_to_keep + 30);

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE ai_document_analyses IS 'AI-powered analysis sessions for document changes and content review';
COMMENT ON TABLE ai_change_analyses IS 'Individual change items detected and analyzed by AI within document analyses';
COMMENT ON TABLE ai_chat_sessions IS 'Conversational AI sessions for document discussion and analysis';
COMMENT ON TABLE ai_chat_messages IS 'Individual messages within AI chat conversations';
COMMENT ON TABLE ai_negotiation_insights IS 'AI-generated insights and recommendations for document negotiation';
COMMENT ON TABLE ai_feedback IS 'User feedback on AI analysis quality and accuracy for model improvement';
COMMENT ON TABLE ai_analysis_triggers IS 'Automated triggers for AI analysis based on document changes or schedules';
COMMENT ON TABLE ai_analysis_notifications IS 'Notifications and alerts generated from AI analysis results';

COMMENT ON COLUMN ai_change_analyses.confidence_score IS 'AI confidence level in the analysis accuracy (0.0 to 1.0)';
COMMENT ON COLUMN ai_change_analyses.risk_factors IS 'JSON array of risk factors identified for this change';
COMMENT ON COLUMN ai_chat_messages.tokens_used IS 'Number of AI tokens consumed for this message (for cost tracking)';
COMMENT ON COLUMN ai_negotiation_insights.historical_data IS 'JSON object containing relevant historical analysis data';
COMMENT ON COLUMN ai_analysis_triggers.trigger_config IS 'JSON configuration for trigger conditions and parameters';