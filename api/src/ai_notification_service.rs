use crate::ai_analysis_service::{ChangeAnalysis, DocumentDiffResponse, ImpactLevel};
use crate::ApiState;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeliveryMethod {
    InApp,
    Email,
    Sms,
    Webhook,
    Slack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationTemplate {
    pub id: String,
    pub name: String,
    pub title_template: String,
    pub message_template: String,
    pub priority: NotificationPriority,
    pub delivery_methods: Vec<DeliveryMethod>,
    pub conditions: NotificationConditions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConditions {
    pub min_impact_level: Option<ImpactLevel>,
    pub max_impact_level: Option<ImpactLevel>,
    pub categories: Option<Vec<String>>,
    pub min_changes_count: Option<i32>,
    pub min_risk_score: Option<f64>,
    pub keywords: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRequest {
    pub template_id: String,
    pub recipient_user_ids: Vec<Uuid>,
    pub context: HashMap<String, serde_json::Value>,
    pub analysis_data: Option<DocumentDiffResponse>,
    pub priority_override: Option<NotificationPriority>,
    pub delivery_method_override: Option<Vec<DeliveryMethod>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRecord {
    pub id: Uuid,
    pub analysis_id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub priority: NotificationPriority,
    pub delivery_method: DeliveryMethod,
    pub is_read: bool,
    pub read_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub delivery_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserNotificationPreferences {
    pub user_id: Uuid,
    pub enabled_notifications: Vec<String>,
    pub delivery_preferences: HashMap<String, Vec<DeliveryMethod>>,
    pub quiet_hours_start: Option<String>, // HH:MM format
    pub quiet_hours_end: Option<String>,
    pub frequency_limits: HashMap<String, i32>, // notification_type -> max per day
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub url: String,
    pub secret: Option<String>,
    pub headers: HashMap<String, String>,
    pub notification_types: Vec<String>,
    pub is_active: bool,
    pub retry_attempts: i32,
    pub timeout_seconds: i32,
}

pub struct AiNotificationService {
    pub pool: PgPool,
    pub email_service: Option<EmailService>,
    pub webhook_service: WebhookService,
    pub templates: HashMap<String, NotificationTemplate>,
}

impl AiNotificationService {
    pub fn new(pool: PgPool) -> Self {
        let mut service = Self {
            pool,
            email_service: None, // Would initialize email service
            webhook_service: WebhookService::new(),
            templates: HashMap::new(),
        };

        service.load_default_templates();
        service
    }

    /// Process analysis completion and send appropriate notifications
    pub async fn process_analysis_completion(
        &self,
        analysis: &DocumentDiffResponse,
    ) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
        let mut notification_ids = Vec::new();

        // Get users with access to this document
        let document_users = self.get_document_users(analysis.document_id).await?;

        // Determine appropriate notification templates based on analysis results
        let templates = self.select_notification_templates(analysis).await?;

        for template_id in templates {
            if let Some(template) = self.templates.get(&template_id) {
                // Check if conditions are met
                if self.evaluate_notification_conditions(&template.conditions, analysis) {
                    let context = self.build_notification_context(analysis);

                    let request = NotificationRequest {
                        template_id: template_id.clone(),
                        recipient_user_ids: document_users.clone(),
                        context,
                        analysis_data: Some(analysis.clone()),
                        priority_override: None,
                        delivery_method_override: None,
                    };

                    let sent_notifications = self.send_notifications(request).await?;
                    notification_ids.extend(sent_notifications);
                }
            }
        }

        Ok(notification_ids)
    }

    /// Send notifications based on request
    pub async fn send_notifications(
        &self,
        request: NotificationRequest,
    ) -> Result<Vec<Uuid>, Box<dyn std::error::Error>> {
        let template = self
            .templates
            .get(&request.template_id)
            .ok_or("Template not found")?;

        let mut notification_ids = Vec::new();

        for user_id in &request.recipient_user_ids {
            // Get user preferences
            let user_prefs = self.get_user_notification_preferences(*user_id).await?;

            // Check if user has enabled this notification type
            if !user_prefs
                .enabled_notifications
                .contains(&request.template_id)
            {
                continue;
            }

            // Check frequency limits
            if self
                .check_frequency_limits(*user_id, &request.template_id)
                .await?
            {
                continue; // Skip if frequency limit exceeded
            }

            // Determine delivery methods
            let delivery_methods = request.delivery_method_override.clone().unwrap_or_else(|| {
                user_prefs
                    .delivery_preferences
                    .get(&request.template_id)
                    .cloned()
                    .unwrap_or(template.delivery_methods.clone())
            });

            // Generate notification content
            let title = self.render_template(&template.title_template, &request.context)?;
            let message = self.render_template(&template.message_template, &request.context)?;

            let priority = request
                .priority_override
                .clone()
                .unwrap_or(template.priority.clone());

            // Send via each delivery method
            for delivery_method in delivery_methods {
                let notification_id = self
                    .create_notification_record(
                        request
                            .analysis_data
                            .as_ref()
                            .map(|a| a.analysis_id)
                            .unwrap_or_else(Uuid::new_v4),
                        *user_id,
                        &request.template_id,
                        &title,
                        &message,
                        &priority,
                        &delivery_method,
                    )
                    .await?;

                // Send the notification
                match self
                    .deliver_notification(
                        notification_id,
                        *user_id,
                        &delivery_method,
                        &title,
                        &message,
                        &request.context,
                    )
                    .await
                {
                    Ok(_) => {
                        self.mark_notification_sent(notification_id).await?;
                        notification_ids.push(notification_id);
                    }
                    Err(e) => {
                        self.mark_notification_failed(notification_id, &e.to_string())
                            .await?;
                        eprintln!("Failed to deliver notification {}: {}", notification_id, e);
                    }
                }
            }
        }

        Ok(notification_ids)
    }

    /// Get unread notifications for a user
    pub async fn get_user_notifications(
        &self,
        user_id: Uuid,
        limit: Option<i32>,
        include_read: bool,
    ) -> Result<Vec<NotificationRecord>, sqlx::Error> {
        let limit = limit.unwrap_or(50);

        let records = sqlx::query!(
            r#"
            SELECT id, analysis_id, user_id, notification_type, title, message,
                   priority, delivery_method, is_read, read_at, sent_at,
                   delivery_status, created_at
            FROM ai_analysis_notifications
            WHERE user_id = $1 AND ($3 OR is_read = false)
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            user_id,
            limit as i64,
            include_read
        )
        .fetch_all(&self.pool)
        .await?;

        let notifications = records
            .into_iter()
            .map(|record| NotificationRecord {
                id: record.id,
                analysis_id: record.analysis_id,
                user_id: record.user_id,
                notification_type: record.notification_type,
                title: record.title,
                message: record.message,
                priority: serde_json::from_str(&record.priority)
                    .unwrap_or(NotificationPriority::Medium),
                delivery_method: record
                    .delivery_method
                    .as_ref()
                    .and_then(|dm| serde_json::from_str(dm).ok())
                    .unwrap_or(DeliveryMethod::InApp),
                is_read: record.is_read,
                read_at: record.read_at,
                sent_at: record.sent_at,
                delivery_status: record.delivery_status,
                created_at: record.created_at.unwrap_or_else(|| chrono::Utc::now()),
            })
            .collect();

        Ok(notifications)
    }

    /// Mark notification as read
    pub async fn mark_notification_read(
        &self,
        notification_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE ai_analysis_notifications
            SET is_read = true, read_at = NOW()
            WHERE id = $1 AND user_id = $2
            "#,
            notification_id,
            user_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update user notification preferences
    pub async fn update_user_preferences(
        &self,
        user_id: Uuid,
        preferences: UserNotificationPreferences,
    ) -> Result<(), sqlx::Error> {
        // TODO: Create user_notification_preferences table
        // For now, just log that preferences would be saved
        println!("Would save notification preferences for user {}", user_id);
        let _ = preferences; // Suppress unused warning

        Ok(())
    }

    // Private helper methods

    async fn get_document_users(&self, document_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
        // TODO: Create document_access table or determine proper user access logic
        // For now, return empty list since we don't know who has access to documents
        let _ = document_id;
        Ok(Vec::new())
    }

    async fn select_notification_templates(
        &self,
        analysis: &DocumentDiffResponse,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let mut templates = Vec::new();

        // Always send analysis completion notification
        templates.push("analysis_complete".to_string());

        // Send critical alert if critical changes found
        if analysis.critical_changes_count > 0 {
            templates.push("critical_changes_alert".to_string());
        }

        // Send high risk alert if risk score is high
        if analysis.overall_risk_score > 0.8 {
            templates.push("high_risk_alert".to_string());
        }

        // Send specific category alerts based on changes
        let financial_changes = analysis.changes.iter().any(|c| {
            matches!(
                c.category,
                crate::ai_analysis_service::ChangeCategory::Financial
            )
        });

        let legal_changes = analysis.changes.iter().any(|c| {
            matches!(
                c.category,
                crate::ai_analysis_service::ChangeCategory::Legal
            )
        });

        if financial_changes {
            templates.push("financial_changes_alert".to_string());
        }

        if legal_changes {
            templates.push("legal_changes_alert".to_string());
        }

        Ok(templates)
    }

    fn evaluate_notification_conditions(
        &self,
        conditions: &NotificationConditions,
        analysis: &DocumentDiffResponse,
    ) -> bool {
        // Check minimum impact level
        if let Some(min_impact) = &conditions.min_impact_level {
            let has_min_impact =
                analysis
                    .changes
                    .iter()
                    .any(|change| match (&change.impact_level, min_impact) {
                        (ImpactLevel::Critical, _) => true,
                        (ImpactLevel::Significant, ImpactLevel::Critical) => false,
                        (ImpactLevel::Significant, _) => true,
                        (
                            ImpactLevel::Moderate,
                            ImpactLevel::Critical | ImpactLevel::Significant,
                        ) => false,
                        (ImpactLevel::Moderate, _) => true,
                        (ImpactLevel::Minor, ImpactLevel::Minor) => true,
                        _ => false,
                    });

            if !has_min_impact {
                return false;
            }
        }

        // Check minimum changes count
        if let Some(min_count) = conditions.min_changes_count {
            if analysis.changes.len() < min_count as usize {
                return false;
            }
        }

        // Check minimum risk score
        if let Some(min_risk) = conditions.min_risk_score {
            if analysis.overall_risk_score < min_risk {
                return false;
            }
        }

        // Check categories
        if let Some(required_categories) = &conditions.categories {
            let analysis_categories: Vec<String> = analysis
                .changes
                .iter()
                .map(|c| format!("{:?}", c.category).to_lowercase())
                .collect();

            let has_required_category = required_categories
                .iter()
                .any(|cat| analysis_categories.contains(&cat.to_lowercase()));

            if !has_required_category {
                return false;
            }
        }

        // Check keywords in summaries
        if let Some(keywords) = &conditions.keywords {
            let has_keyword = analysis.changes.iter().any(|change| {
                keywords.iter().any(|keyword| {
                    change
                        .summary
                        .to_lowercase()
                        .contains(&keyword.to_lowercase())
                })
            });

            if !has_keyword {
                return false;
            }
        }

        true
    }

    fn build_notification_context(
        &self,
        analysis: &DocumentDiffResponse,
    ) -> HashMap<String, serde_json::Value> {
        let mut context = HashMap::new();

        context.insert(
            "document_id".to_string(),
            serde_json::json!(analysis.document_id),
        );
        context.insert(
            "analysis_id".to_string(),
            serde_json::json!(analysis.analysis_id),
        );
        context.insert(
            "total_changes".to_string(),
            serde_json::json!(analysis.changes.len()),
        );
        context.insert(
            "critical_changes".to_string(),
            serde_json::json!(analysis.critical_changes_count),
        );
        context.insert(
            "risk_score".to_string(),
            serde_json::json!(analysis.overall_risk_score),
        );
        context.insert("summary".to_string(), serde_json::json!(analysis.summary));
        context.insert(
            "processed_at".to_string(),
            serde_json::json!(analysis.processed_at),
        );

        // Add change categories breakdown
        let mut category_counts = HashMap::new();
        for change in &analysis.changes {
            let category = format!("{:?}", change.category).to_lowercase();
            *category_counts.entry(category).or_insert(0) += 1;
        }
        context.insert(
            "category_breakdown".to_string(),
            serde_json::json!(category_counts),
        );

        context
    }

    async fn get_user_notification_preferences(
        &self,
        user_id: Uuid,
    ) -> Result<UserNotificationPreferences, sqlx::Error> {
        // TODO: Create user_notification_preferences table
        // For now, return default preferences for all users
        Ok(UserNotificationPreferences {
            user_id,
            enabled_notifications: vec![
                "analysis_complete".to_string(),
                "critical_changes_alert".to_string(),
            ],
            delivery_preferences: HashMap::new(),
            quiet_hours_start: None,
            quiet_hours_end: None,
            frequency_limits: HashMap::new(),
        })
    }

    async fn check_frequency_limits(
        &self,
        user_id: Uuid,
        notification_type: &str,
    ) -> Result<bool, sqlx::Error> {
        // Check how many notifications of this type were sent today
        let count = sqlx::query!(
            r#"
            SELECT COUNT(*)
            FROM ai_analysis_notifications
            WHERE user_id = $1
            AND notification_type = $2
            AND created_at >= CURRENT_DATE
            "#,
            user_id,
            notification_type
        )
        .fetch_one(&self.pool)
        .await?
        .count
        .unwrap_or(0);

        // Get user's frequency limit for this notification type
        let user_prefs = self.get_user_notification_preferences(user_id).await?;
        let limit = user_prefs
            .frequency_limits
            .get(notification_type)
            .copied()
            .unwrap_or(10); // Default limit of 10 per day

        Ok(count >= limit as i64)
    }

    fn render_template(
        &self,
        template: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut rendered = template.to_string();

        // Simple template rendering - replace {{key}} with values
        for (key, value) in context {
            let placeholder = format!("{{{{{}}}}}", key);
            let replacement = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => value.to_string(),
            };
            rendered = rendered.replace(&placeholder, &replacement);
        }

        Ok(rendered)
    }

    async fn create_notification_record(
        &self,
        analysis_id: Uuid,
        user_id: Uuid,
        notification_type: &str,
        title: &str,
        message: &str,
        priority: &NotificationPriority,
        delivery_method: &DeliveryMethod,
    ) -> Result<Uuid, sqlx::Error> {
        let notification_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO ai_analysis_notifications (
                id, analysis_id, user_id, notification_type, title, message,
                priority, delivery_method, is_read, delivery_status, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, false, 'pending', NOW())
            "#,
            notification_id,
            analysis_id,
            user_id,
            notification_type,
            title,
            message,
            serde_json::to_string(priority).unwrap_or_else(|_| "medium".to_string()),
            serde_json::to_string(delivery_method).unwrap_or_else(|_| "in_app".to_string())
        )
        .execute(&self.pool)
        .await?;

        Ok(notification_id)
    }

    async fn deliver_notification(
        &self,
        notification_id: Uuid,
        user_id: Uuid,
        delivery_method: &DeliveryMethod,
        title: &str,
        message: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match delivery_method {
            DeliveryMethod::InApp => {
                // In-app notifications are already stored in database
                Ok(())
            }
            DeliveryMethod::Email => {
                if let Some(email_service) = &self.email_service {
                    email_service.send_email(user_id, title, message).await
                } else {
                    Ok(()) // Email service not configured
                }
            }
            DeliveryMethod::Webhook => {
                self.webhook_service
                    .send_webhook(user_id, title, message, context)
                    .await
            }
            DeliveryMethod::Sms => {
                // Would integrate with SMS service
                Ok(())
            }
            DeliveryMethod::Slack => {
                // Would integrate with Slack API
                Ok(())
            }
        }
    }

    async fn mark_notification_sent(&self, notification_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE ai_analysis_notifications
            SET delivery_status = 'sent', sent_at = NOW()
            WHERE id = $1
            "#,
            notification_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn mark_notification_failed(
        &self,
        notification_id: Uuid,
        error: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE ai_analysis_notifications
            SET delivery_status = 'failed'
            WHERE id = $1
            "#,
            notification_id
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn load_default_templates(&mut self) {
        // Analysis complete template
        self.templates.insert(
            "analysis_complete".to_string(),
            NotificationTemplate {
                id: "analysis_complete".to_string(),
                name: "Analysis Complete".to_string(),
                title_template: "Document Analysis Complete".to_string(),
                message_template: "AI analysis found {{total_changes}} changes with a risk score of {{risk_score}}. {{summary}}".to_string(),
                priority: NotificationPriority::Medium,
                delivery_methods: vec![DeliveryMethod::InApp],
                conditions: NotificationConditions {
                    min_impact_level: None,
                    max_impact_level: None,
                    categories: None,
                    min_changes_count: Some(1),
                    min_risk_score: None,
                    keywords: None,
                },
            },
        );

        // Critical changes alert template
        self.templates.insert(
            "critical_changes_alert".to_string(),
            NotificationTemplate {
                id: "critical_changes_alert".to_string(),
                name: "Critical Changes Alert".to_string(),
                title_template: "🚨 Critical Document Changes Detected".to_string(),
                message_template: "{{critical_changes}} critical changes detected in your document. Immediate review recommended. Risk score: {{risk_score}}".to_string(),
                priority: NotificationPriority::Urgent,
                delivery_methods: vec![DeliveryMethod::InApp, DeliveryMethod::Email],
                conditions: NotificationConditions {
                    min_impact_level: Some(ImpactLevel::Critical),
                    max_impact_level: None,
                    categories: None,
                    min_changes_count: None,
                    min_risk_score: None,
                    keywords: None,
                },
            },
        );

        // High risk alert template
        self.templates.insert(
            "high_risk_alert".to_string(),
            NotificationTemplate {
                id: "high_risk_alert".to_string(),
                name: "High Risk Alert".to_string(),
                title_template: "⚠️ High Risk Changes Detected".to_string(),
                message_template: "Document changes show high risk score of {{risk_score}}. Consider legal review before proceeding.".to_string(),
                priority: NotificationPriority::High,
                delivery_methods: vec![DeliveryMethod::InApp, DeliveryMethod::Email],
                conditions: NotificationConditions {
                    min_impact_level: None,
                    max_impact_level: None,
                    categories: None,
                    min_changes_count: None,
                    min_risk_score: Some(0.8),
                    keywords: None,
                },
            },
        );

        // Financial changes alert
        self.templates.insert(
            "financial_changes_alert".to_string(),
            NotificationTemplate {
                id: "financial_changes_alert".to_string(),
                name: "Financial Changes Alert".to_string(),
                title_template: "💰 Financial Terms Modified".to_string(),
                message_template: "Financial terms in your document have been modified. Review impact on deal economics.".to_string(),
                priority: NotificationPriority::High,
                delivery_methods: vec![DeliveryMethod::InApp],
                conditions: NotificationConditions {
                    min_impact_level: Some(ImpactLevel::Moderate),
                    max_impact_level: None,
                    categories: Some(vec!["financial".to_string()]),
                    min_changes_count: None,
                    min_risk_score: None,
                    keywords: None,
                },
            },
        );

        // Legal changes alert
        self.templates.insert(
            "legal_changes_alert".to_string(),
            NotificationTemplate {
                id: "legal_changes_alert".to_string(),
                name: "Legal Changes Alert".to_string(),
                title_template: "⚖️ Legal Terms Modified".to_string(),
                message_template:
                    "Legal terms in your document have been modified. Legal review may be required."
                        .to_string(),
                priority: NotificationPriority::High,
                delivery_methods: vec![DeliveryMethod::InApp],
                conditions: NotificationConditions {
                    min_impact_level: Some(ImpactLevel::Moderate),
                    max_impact_level: None,
                    categories: Some(vec!["legal".to_string()]),
                    min_changes_count: None,
                    min_risk_score: None,
                    keywords: None,
                },
            },
        );
    }
}

// Helper services

pub struct EmailService {
    // Email service implementation would go here
}

impl EmailService {
    pub async fn send_email(
        &self,
        _user_id: Uuid,
        _title: &str,
        _message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Email sending implementation
        Ok(())
    }
}

pub struct WebhookService {
    client: reqwest::Client,
}

impl WebhookService {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn send_webhook(
        &self,
        user_id: Uuid,
        title: &str,
        message: &str,
        context: &HashMap<String, serde_json::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Webhook implementation would get user's webhook configs and send
        // This is a simplified version
        let payload = serde_json::json!({
            "user_id": user_id,
            "title": title,
            "message": message,
            "context": context,
            "timestamp": chrono::Utc::now()
        });

        // Would iterate through user's webhook configs and send to each
        println!("Webhook payload: {}", payload);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_analysis_service::{ChangeAnalysis, ChangeCategory, ChangeType, ImpactLevel};
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_template_rendering() {
        // Create a mock pool for testing (this won't actually connect)
        let database_url = "postgresql://test:test@localhost:5432/test_db";
        let pool = match sqlx::PgPool::connect(database_url).await {
            Ok(p) => p,
            Err(_) => {
                // Skip test if database not available
                return;
            }
        };
        let service = AiNotificationService::new(pool);

        let mut context = HashMap::new();
        context.insert("total_changes".to_string(), serde_json::json!(5));
        context.insert("risk_score".to_string(), serde_json::json!(0.75));

        let template = "Found {{total_changes}} changes with risk {{risk_score}}";
        let rendered = service.render_template(template, &context).unwrap();

        assert_eq!(rendered, "Found 5 changes with risk 0.75");
    }

    #[tokio::test]
    async fn test_notification_conditions_evaluation() {
        let database_url = "postgresql://test:test@localhost:5432/test_db";
        let pool = match sqlx::PgPool::connect(database_url).await {
            Ok(p) => p,
            Err(_) => {
                // Skip test if database not available
                return;
            }
        };
        let service = AiNotificationService::new(pool);

        let conditions = NotificationConditions {
            min_impact_level: Some(ImpactLevel::Significant),
            max_impact_level: None,
            categories: None,
            min_changes_count: Some(2),
            min_risk_score: Some(0.6),
            keywords: None,
        };

        let analysis = DocumentDiffResponse {
            document_id: Uuid::new_v4(),
            analysis_id: Uuid::new_v4(),
            changes: vec![ChangeAnalysis {
                id: Uuid::new_v4(),
                document_id: Uuid::new_v4(),
                change_type: ChangeType::Addition,
                impact_level: ImpactLevel::Critical,
                category: ChangeCategory::Financial,
                summary: "Test change".to_string(),
                implications: vec![],
                recommendations: vec![],
                related_clauses: vec![],
                risk_factors: vec![],
                confidence_score: 0.8,
                created_at: chrono::Utc::now(),
            }],
            overall_risk_score: 0.7,
            summary: "Test".to_string(),
            critical_changes_count: 1,
            processed_at: chrono::Utc::now(),
        };

        let should_notify = service.evaluate_notification_conditions(&conditions, &analysis);

        // Should NOT notify because we need at least 2 changes
        assert!(!should_notify);
    }
}
