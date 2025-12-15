-- Drop AI-related tables and notification system
-- This removes the unused AI analysis system that was never integrated into the active API

-- ============================================================================
-- DROP NOTIFICATION DELIVERY TABLES (children first)
-- ============================================================================

-- Drop delivery log tables that reference notification_templates and ai_analysis_notifications
DROP TABLE IF EXISTS webhook_delivery_logs CASCADE;
DROP TABLE IF EXISTS email_delivery_logs CASCADE;
DROP TABLE IF EXISTS sms_delivery_logs CASCADE;

-- Drop user customization tables
DROP TABLE IF EXISTS user_notification_templates CASCADE;
DROP TABLE IF EXISTS user_notification_preferences CASCADE;
DROP TABLE IF EXISTS user_webhook_configs CASCADE;

-- Drop notification templates table
DROP TABLE IF EXISTS notification_templates CASCADE;

-- ============================================================================
-- DROP AI ANALYSIS TABLES (children first, then parents)
-- ============================================================================

-- Drop AI feedback table (references ai_document_analyses, ai_chat_messages, ai_negotiation_insights)
DROP TABLE IF EXISTS ai_feedback CASCADE;

-- Drop AI analysis notifications (references ai_document_analyses)
DROP TABLE IF EXISTS ai_analysis_notifications CASCADE;

-- Drop AI analysis triggers
DROP TABLE IF EXISTS ai_analysis_triggers CASCADE;

-- Drop AI negotiation insights (references ai_document_analyses)
DROP TABLE IF EXISTS ai_negotiation_insights CASCADE;

-- Drop AI chat messages (references ai_chat_sessions, ai_document_analyses)
DROP TABLE IF EXISTS ai_chat_messages CASCADE;

-- Drop AI chat sessions
DROP TABLE IF EXISTS ai_chat_sessions CASCADE;

-- Drop AI change analyses (references ai_document_analyses)
DROP TABLE IF EXISTS ai_change_analyses CASCADE;

-- Drop AI document analyses (parent table)
DROP TABLE IF EXISTS ai_document_analyses CASCADE;

-- ============================================================================
-- DROP HELPER FUNCTIONS
-- ============================================================================

-- Drop AI-related helper functions
DROP FUNCTION IF EXISTS get_document_ai_stats(UUID);
DROP FUNCTION IF EXISTS get_user_priority_insights(UUID, INTEGER);
DROP FUNCTION IF EXISTS cleanup_old_ai_chat_data(INTEGER);
DROP FUNCTION IF EXISTS update_chat_session_activity();
DROP FUNCTION IF EXISTS get_effective_notification_preferences(UUID);
DROP FUNCTION IF EXISTS is_user_in_quiet_hours(UUID);
DROP FUNCTION IF EXISTS cleanup_old_delivery_logs(INTEGER);

-- ============================================================================
-- VERIFICATION
-- ============================================================================

-- Note: All indexes and triggers are automatically dropped with CASCADE
-- No need to explicitly drop them

COMMENT ON SCHEMA public IS 'AI analysis tables and notification system removed - unused feature never integrated into active API';

