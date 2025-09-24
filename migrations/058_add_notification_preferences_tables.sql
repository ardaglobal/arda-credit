-- User Notification Preferences and Webhook Configuration
-- This migration adds tables for managing user notification preferences,
-- webhook configurations, and notification delivery tracking.

-- ============================================================================
-- USER NOTIFICATION PREFERENCES
-- ============================================================================

-- Table for storing user-specific notification preferences
CREATE TABLE user_notification_preferences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    enabled_notifications JSONB NOT NULL DEFAULT '[]'::jsonb, -- Array of enabled notification types
    delivery_preferences JSONB NOT NULL DEFAULT '{}'::jsonb,  -- Map of notification_type -> delivery methods
    quiet_hours_start TIME,                                   -- Start of quiet hours (HH:MM)
    quiet_hours_end TIME,                                     -- End of quiet hours (HH:MM)
    timezone VARCHAR(50) DEFAULT 'UTC',                       -- User's timezone for quiet hours
    frequency_limits JSONB NOT NULL DEFAULT '{}'::jsonb,      -- Map of notification_type -> max per day
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(user_id)
);

-- ============================================================================
-- WEBHOOK CONFIGURATIONS
-- ============================================================================

-- Table for storing webhook configurations for users/organizations
CREATE TABLE user_webhook_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    url TEXT NOT NULL,
    secret VARCHAR(255),                                       -- Webhook secret for signature verification
    headers JSONB DEFAULT '{}'::jsonb,                        -- Custom headers to send with webhook
    notification_types JSONB NOT NULL DEFAULT '[]'::jsonb,    -- Array of notification types to send
    is_active BOOLEAN NOT NULL DEFAULT true,
    retry_attempts INTEGER NOT NULL DEFAULT 3,
    timeout_seconds INTEGER NOT NULL DEFAULT 30,
    last_success_at TIMESTAMP WITH TIME ZONE,
    last_failure_at TIMESTAMP WITH TIME ZONE,
    failure_count INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CHECK (timeout_seconds > 0 AND timeout_seconds <= 300),
    CHECK (retry_attempts >= 0 AND retry_attempts <= 10)
);

-- ============================================================================
-- NOTIFICATION DELIVERY TRACKING
-- ============================================================================

-- Table for tracking webhook delivery attempts and results
CREATE TABLE webhook_delivery_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    webhook_config_id UUID NOT NULL REFERENCES user_webhook_configs(id) ON DELETE CASCADE,
    notification_id UUID NOT NULL REFERENCES ai_analysis_notifications(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL DEFAULT 1,
    request_url TEXT NOT NULL,
    request_headers JSONB,
    request_body JSONB,
    response_status INTEGER,
    response_headers JSONB,
    response_body TEXT,
    error_message TEXT,
    delivery_duration_ms INTEGER,
    success BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CHECK (attempt_number > 0),
    CHECK (response_status IS NULL OR (response_status >= 100 AND response_status < 600))
);

-- Table for email delivery tracking (if email service is used)
CREATE TABLE email_delivery_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID NOT NULL REFERENCES ai_analysis_notifications(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    email_address VARCHAR(255) NOT NULL,
    subject VARCHAR(500) NOT NULL,
    email_provider VARCHAR(50),                               -- sendgrid, ses, smtp, etc.
    provider_message_id VARCHAR(255),                         -- Provider-specific message ID
    delivery_status VARCHAR(20) NOT NULL DEFAULT 'pending',   -- pending, sent, delivered, bounced, failed
    error_message TEXT,
    sent_at TIMESTAMP WITH TIME ZONE,
    delivered_at TIMESTAMP WITH TIME ZONE,
    opened_at TIMESTAMP WITH TIME ZONE,                       -- If tracking pixels are used
    clicked_at TIMESTAMP WITH TIME ZONE,                      -- If click tracking is used
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CHECK (delivery_status IN ('pending', 'sent', 'delivered', 'bounced', 'failed', 'spam'))
);

-- Table for SMS delivery tracking (if SMS service is used)
CREATE TABLE sms_delivery_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    notification_id UUID NOT NULL REFERENCES ai_analysis_notifications(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    phone_number VARCHAR(20) NOT NULL,
    message TEXT NOT NULL,
    sms_provider VARCHAR(50),                                 -- twilio, aws sns, etc.
    provider_message_id VARCHAR(255),
    delivery_status VARCHAR(20) NOT NULL DEFAULT 'pending',   -- pending, sent, delivered, failed, undelivered
    error_message TEXT,
    cost_cents INTEGER,                                       -- Cost in cents for billing tracking
    sent_at TIMESTAMP WITH TIME ZONE,
    delivered_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CHECK (delivery_status IN ('pending', 'sent', 'delivered', 'failed', 'undelivered'))
);

-- ============================================================================
-- NOTIFICATION TEMPLATES STORAGE
-- ============================================================================

-- Table for storing customizable notification templates
CREATE TABLE notification_templates (
    id VARCHAR(100) PRIMARY KEY,                             -- e.g., 'analysis_complete', 'critical_changes_alert'
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category VARCHAR(50) NOT NULL,                           -- ai_analysis, system, user_action, etc.
    title_template TEXT NOT NULL,
    message_template TEXT NOT NULL,
    email_template TEXT,                                     -- HTML template for emails
    sms_template TEXT,                                       -- Shorter template for SMS
    webhook_template JSONB,                                  -- JSON template for webhook payloads
    default_priority VARCHAR(20) NOT NULL DEFAULT 'medium',  -- low, medium, high, urgent
    default_delivery_methods JSONB NOT NULL DEFAULT '["in_app"]'::jsonb,
    conditions JSONB DEFAULT '{}'::jsonb,                    -- Conditions for when to send
    is_system_template BOOLEAN NOT NULL DEFAULT true,       -- System vs user-customizable
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    CHECK (default_priority IN ('low', 'medium', 'high', 'urgent'))
);

-- ============================================================================
-- USER CUSTOMIZABLE TEMPLATES
-- ============================================================================

-- Table for user-specific template customizations
CREATE TABLE user_notification_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id) ON DELETE CASCADE,
    template_id VARCHAR(100) NOT NULL REFERENCES notification_templates(id) ON DELETE CASCADE,
    title_template TEXT,                                     -- User's custom title template
    message_template TEXT,                                   -- User's custom message template
    email_template TEXT,                                     -- User's custom email template
    priority_override VARCHAR(20),                          -- User's priority preference
    delivery_methods_override JSONB,                        -- User's delivery method preference
    is_enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),

    UNIQUE(user_id, template_id),
    CHECK (priority_override IS NULL OR priority_override IN ('low', 'medium', 'high', 'urgent'))
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- User notification preferences indexes
CREATE INDEX idx_user_notification_preferences_user_id ON user_notification_preferences(user_id);

-- Webhook configs indexes
CREATE INDEX idx_user_webhook_configs_user_id ON user_webhook_configs(user_id);
CREATE INDEX idx_user_webhook_configs_active ON user_webhook_configs(is_active);
CREATE INDEX idx_user_webhook_configs_failures ON user_webhook_configs(failure_count) WHERE failure_count > 0;

-- Webhook delivery logs indexes
CREATE INDEX idx_webhook_delivery_logs_webhook_config_id ON webhook_delivery_logs(webhook_config_id);
CREATE INDEX idx_webhook_delivery_logs_notification_id ON webhook_delivery_logs(notification_id);
CREATE INDEX idx_webhook_delivery_logs_success ON webhook_delivery_logs(success, created_at);
CREATE INDEX idx_webhook_delivery_logs_created_at ON webhook_delivery_logs(created_at DESC);

-- Email delivery logs indexes
CREATE INDEX idx_email_delivery_logs_notification_id ON email_delivery_logs(notification_id);
CREATE INDEX idx_email_delivery_logs_user_id ON email_delivery_logs(user_id);
CREATE INDEX idx_email_delivery_logs_status ON email_delivery_logs(delivery_status);
CREATE INDEX idx_email_delivery_logs_sent_at ON email_delivery_logs(sent_at) WHERE sent_at IS NOT NULL;

-- SMS delivery logs indexes
CREATE INDEX idx_sms_delivery_logs_notification_id ON sms_delivery_logs(notification_id);
CREATE INDEX idx_sms_delivery_logs_user_id ON sms_delivery_logs(user_id);
CREATE INDEX idx_sms_delivery_logs_status ON sms_delivery_logs(delivery_status);
CREATE INDEX idx_sms_delivery_logs_cost ON sms_delivery_logs(cost_cents) WHERE cost_cents IS NOT NULL;

-- Notification templates indexes
CREATE INDEX idx_notification_templates_category ON notification_templates(category);
CREATE INDEX idx_notification_templates_active ON notification_templates(is_active);

-- User notification templates indexes
CREATE INDEX idx_user_notification_templates_user_id ON user_notification_templates(user_id);
CREATE INDEX idx_user_notification_templates_template_id ON user_notification_templates(template_id);
CREATE INDEX idx_user_notification_templates_enabled ON user_notification_templates(is_enabled);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update user_notification_preferences.updated_at on changes
CREATE TRIGGER update_user_notification_preferences_updated_at
    BEFORE UPDATE ON user_notification_preferences
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Update user_webhook_configs.updated_at on changes
CREATE TRIGGER update_user_webhook_configs_updated_at
    BEFORE UPDATE ON user_webhook_configs
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Update notification_templates.updated_at on changes
CREATE TRIGGER update_notification_templates_updated_at
    BEFORE UPDATE ON notification_templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- Update user_notification_templates.updated_at on changes
CREATE TRIGGER update_user_notification_templates_updated_at
    BEFORE UPDATE ON user_notification_templates
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ============================================================================
-- HELPER FUNCTIONS
-- ============================================================================

-- Function to get user's effective notification preferences (with defaults)
CREATE OR REPLACE FUNCTION get_effective_notification_preferences(user_id_param UUID)
RETURNS TABLE(
    template_id VARCHAR(100),
    title_template TEXT,
    message_template TEXT,
    priority VARCHAR(20),
    delivery_methods JSONB,
    is_enabled BOOLEAN
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        nt.id as template_id,
        COALESCE(unt.title_template, nt.title_template) as title_template,
        COALESCE(unt.message_template, nt.message_template) as message_template,
        COALESCE(unt.priority_override, nt.default_priority) as priority,
        COALESCE(unt.delivery_methods_override, nt.default_delivery_methods) as delivery_methods,
        COALESCE(unt.is_enabled, true) as is_enabled
    FROM notification_templates nt
    LEFT JOIN user_notification_templates unt ON nt.id = unt.template_id AND unt.user_id = user_id_param
    WHERE nt.is_active = true
    ORDER BY nt.category, nt.id;
END;
$$ LANGUAGE plpgsql;

-- Function to check if user is in quiet hours
CREATE OR REPLACE FUNCTION is_user_in_quiet_hours(user_id_param UUID)
RETURNS BOOLEAN AS $$
DECLARE
    user_prefs RECORD;
    current_user_time TIME;
    user_tz TEXT;
BEGIN
    -- Get user's quiet hours preferences
    SELECT quiet_hours_start, quiet_hours_end, timezone
    INTO user_prefs
    FROM user_notification_preferences
    WHERE user_id = user_id_param;

    -- If no preferences set, not in quiet hours
    IF user_prefs IS NULL OR user_prefs.quiet_hours_start IS NULL OR user_prefs.quiet_hours_end IS NULL THEN
        RETURN FALSE;
    END IF;

    -- Get current time in user's timezone
    user_tz := COALESCE(user_prefs.timezone, 'UTC');
    current_user_time := (NOW() AT TIME ZONE user_tz)::TIME;

    -- Check if current time falls within quiet hours
    IF user_prefs.quiet_hours_start <= user_prefs.quiet_hours_end THEN
        -- Same day quiet hours (e.g., 22:00 to 08:00 next day)
        RETURN current_user_time BETWEEN user_prefs.quiet_hours_start AND user_prefs.quiet_hours_end;
    ELSE
        -- Cross-midnight quiet hours (e.g., 22:00 to 08:00)
        RETURN current_user_time >= user_prefs.quiet_hours_start OR current_user_time <= user_prefs.quiet_hours_end;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Function to clean up old delivery logs
CREATE OR REPLACE FUNCTION cleanup_old_delivery_logs(days_to_keep INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER := 0;
    temp_count INTEGER;
BEGIN
    -- Clean up webhook delivery logs
    DELETE FROM webhook_delivery_logs
    WHERE created_at < NOW() - INTERVAL '%d days' * days_to_keep;
    GET DIAGNOSTICS temp_count = ROW_COUNT;
    deleted_count := deleted_count + temp_count;

    -- Clean up email delivery logs
    DELETE FROM email_delivery_logs
    WHERE created_at < NOW() - INTERVAL '%d days' * days_to_keep;
    GET DIAGNOSTICS temp_count = ROW_COUNT;
    deleted_count := deleted_count + temp_count;

    -- Clean up SMS delivery logs
    DELETE FROM sms_delivery_logs
    WHERE created_at < NOW() - INTERVAL '%d days' * days_to_keep;
    GET DIAGNOSTICS temp_count = ROW_COUNT;
    deleted_count := deleted_count + temp_count;

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- ============================================================================
-- INSERT DEFAULT NOTIFICATION TEMPLATES
-- ============================================================================

-- Insert default system notification templates
INSERT INTO notification_templates (
    id, name, description, category, title_template, message_template,
    default_priority, default_delivery_methods, conditions
) VALUES
(
    'analysis_complete',
    'Analysis Complete',
    'Notification sent when AI document analysis is completed',
    'ai_analysis',
    'Document Analysis Complete',
    'AI analysis found {{total_changes}} changes with a risk score of {{risk_score}}. {{summary}}',
    'medium',
    '["in_app"]'::jsonb,
    '{"min_changes_count": 1}'::jsonb
),
(
    'critical_changes_alert',
    'Critical Changes Alert',
    'Urgent notification for critical document changes',
    'ai_analysis',
    '🚨 Critical Document Changes Detected',
    '{{critical_changes}} critical changes detected in your document. Immediate review recommended. Risk score: {{risk_score}}',
    'urgent',
    '["in_app", "email"]'::jsonb,
    '{"min_impact_level": "critical"}'::jsonb
),
(
    'high_risk_alert',
    'High Risk Alert',
    'Alert for high-risk document changes',
    'ai_analysis',
    '⚠️ High Risk Changes Detected',
    'Document changes show high risk score of {{risk_score}}. Consider legal review before proceeding.',
    'high',
    '["in_app", "email"]'::jsonb,
    '{"min_risk_score": 0.8}'::jsonb
),
(
    'financial_changes_alert',
    'Financial Changes Alert',
    'Alert for modifications to financial terms',
    'ai_analysis',
    '💰 Financial Terms Modified',
    'Financial terms in your document have been modified. Review impact on deal economics.',
    'high',
    '["in_app"]'::jsonb,
    '{"categories": ["financial"], "min_impact_level": "moderate"}'::jsonb
),
(
    'legal_changes_alert',
    'Legal Changes Alert',
    'Alert for modifications to legal terms',
    'ai_analysis',
    '⚖️ Legal Terms Modified',
    'Legal terms in your document have been modified. Legal review may be required.',
    'high',
    '["in_app"]'::jsonb,
    '{"categories": ["legal"], "min_impact_level": "moderate"}'::jsonb
),
(
    'daily_analysis_summary',
    'Daily Analysis Summary',
    'Daily summary of all document analysis activity',
    'ai_analysis',
    'Daily Analysis Report - {{date}}',
    'Today processed {{total_analyses}} document analyses with {{critical_count}} critical changes across {{document_count}} documents.',
    'low',
    '["email"]'::jsonb,
    '{"schedule": "daily"}'::jsonb
);

-- ============================================================================
-- TABLE COMMENTS
-- ============================================================================

COMMENT ON TABLE user_notification_preferences IS 'User-specific preferences for notification delivery and timing';
COMMENT ON TABLE user_webhook_configs IS 'Webhook endpoints configured by users for receiving notifications';
COMMENT ON TABLE webhook_delivery_logs IS 'Log of webhook delivery attempts and results for debugging';
COMMENT ON TABLE email_delivery_logs IS 'Log of email notification deliveries and tracking';
COMMENT ON TABLE sms_delivery_logs IS 'Log of SMS notification deliveries and cost tracking';
COMMENT ON TABLE notification_templates IS 'System-wide notification templates with customizable content';
COMMENT ON TABLE user_notification_templates IS 'User-specific customizations of system notification templates';

COMMENT ON COLUMN user_notification_preferences.enabled_notifications IS 'JSON array of notification template IDs that user has enabled';
COMMENT ON COLUMN user_notification_preferences.delivery_preferences IS 'JSON object mapping notification types to preferred delivery methods';
COMMENT ON COLUMN user_notification_preferences.frequency_limits IS 'JSON object mapping notification types to maximum daily counts';
COMMENT ON COLUMN user_webhook_configs.secret IS 'Secret key for HMAC signature verification of webhook payloads';
COMMENT ON COLUMN notification_templates.conditions IS 'JSON conditions that must be met for template to trigger';
COMMENT ON COLUMN notification_templates.webhook_template IS 'JSON template structure for webhook payload formatting';