use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChangeType {
    Addition,
    Deletion,
    Modification,
    Restructure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ImpactLevel {
    Minor,
    Moderate,
    Significant,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChangeCategory {
    Financial,
    Legal,
    Operational,
    Compliance,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RiskFactor {
    pub risk_type: String,
    pub severity: ImpactLevel,
    pub description: String,
    pub mitigation_suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChangeAnalysis {
    pub id: Uuid,
    pub document_id: Uuid,
    pub change_type: ChangeType,
    pub impact_level: ImpactLevel,
    pub category: ChangeCategory,
    pub summary: String,
    pub implications: Vec<String>,
    pub recommendations: Vec<String>,
    pub related_clauses: Vec<String>,
    pub risk_factors: Vec<RiskFactor>,
    pub confidence_score: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DocumentDiffRequest {
    pub document_id: Uuid,
    pub previous_version_id: Option<Uuid>,
    pub current_version_id: Uuid,
    pub analysis_type: Option<String>,
    pub focus_areas: Option<Vec<ChangeCategory>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DocumentDiffResponse {
    pub document_id: Uuid,
    pub analysis_id: Uuid,
    pub changes: Vec<ChangeAnalysis>,
    pub overall_risk_score: f64,
    pub summary: String,
    pub critical_changes_count: i32,
    pub processed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiChatMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub document_id: Option<Uuid>,
    pub user_id: Uuid,
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub context: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiChatRequest {
    pub session_id: Option<Uuid>,
    pub document_id: Option<Uuid>,
    pub message: String,
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AiChatResponse {
    pub session_id: Uuid,
    pub message_id: Uuid,
    pub response: String,
    pub suggestions: Vec<String>,
    pub related_analyses: Vec<Uuid>,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NegotiationInsight {
    pub id: Uuid,
    pub document_id: Uuid,
    pub insight_type: String,
    pub title: String,
    pub description: String,
    pub market_comparison: Option<String>,
    pub historical_data: Option<serde_json::Value>,
    pub recommended_action: Option<String>,
    pub priority: ImpactLevel,
    pub created_at: DateTime<Utc>,
}

/// Request for generating insights with a custom prompt
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InsightsRequest {
    /// Custom prompt for the AI analysis
    pub prompt: String,
    /// Optional company_id for multi-tenant search scoping
    pub company_id: Option<Uuid>,
    /// Maximum number of relevant chunks to retrieve (default: 10)
    pub max_chunks: Option<usize>,
}

/// BAML-formatted insight response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BAMLInsightResponse {
    /// Unique insight ID
    pub insight_id: Uuid,
    /// Document being analyzed
    pub document_id: Uuid,
    /// Type of insight (e.g., "opportunity", "risk", "attention_area")
    pub insight_type: String,
    /// Main insight title
    pub title: String,
    /// Detailed description of the insight
    pub description: String,
    /// Relevant document excerpts that support this insight
    pub supporting_evidence: Vec<EvidenceChunk>,
    /// Recommended actions based on this insight
    pub recommended_actions: Vec<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Impact level
    pub impact: ImpactLevel,
    /// When this insight was generated
    pub generated_at: DateTime<Utc>,
}

/// Evidence chunk from the document
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct EvidenceChunk {
    /// Chunk text excerpt
    pub text: String,
    /// Page number if available
    pub page_number: Option<i32>,
    /// Relevance score (0.0 to 1.0)
    pub relevance_score: f32,
}

/// Request payload for Modal agent
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModalInsightsRequest {
    document_id: String,
    prompt: String,
    company_id: Option<String>,
    max_chunks: usize,
}

/// Response from Modal agent
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModalInsightsResponse {
    insights: Vec<ModalInsight>,
}

/// Individual insight from Modal agent
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModalInsight {
    insight_type: String,
    title: String,
    description: String,
    supporting_evidence: Vec<ModalEvidenceChunk>,
    recommended_actions: Vec<String>,
    confidence: f64,
    impact: String,
}

/// Evidence chunk from Modal agent
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModalEvidenceChunk {
    text: String,
    page_number: Option<i32>,
    relevance_score: f32,
}

/// Parse impact level string to enum
fn parse_impact_level(impact: &str) -> ImpactLevel {
    match impact.to_lowercase().as_str() {
        "critical" => ImpactLevel::Critical,
        "significant" => ImpactLevel::Significant,
        "moderate" => ImpactLevel::Moderate,
        "minor" => ImpactLevel::Minor,
        _ => ImpactLevel::Minor,
    }
}

pub struct AiAnalysisService {
    pub pool: PgPool,
}

impl AiAnalysisService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Analyze differences between document versions using AI
    pub async fn analyze_document_diff(
        &self,
        request: DocumentDiffRequest,
    ) -> Result<DocumentDiffResponse, anyhow::Error> {
        // Get document content for both versions
        let current_content = self
            .get_document_content(request.current_version_id)
            .await?;
        let previous_content = match request.previous_version_id {
            Some(prev_id) => Some(self.get_document_content(prev_id).await?),
            None => None,
        };

        // Perform AI analysis
        let changes = self
            .perform_ai_diff_analysis(
                &current_content,
                previous_content.as_ref(),
                &request.focus_areas,
            )
            .await?;

        // Calculate overall risk score
        let overall_risk_score = self.calculate_overall_risk_score(&changes);
        let critical_changes_count = changes
            .iter()
            .filter(|c| c.impact_level == ImpactLevel::Critical)
            .count() as i32;

        // Generate summary
        let summary = self.generate_analysis_summary(&changes).await?;

        // Save analysis to database
        let analysis_id = Uuid::new_v4();
        self.save_analysis_results(analysis_id, &request, &changes)
            .await?;

        Ok(DocumentDiffResponse {
            document_id: request.document_id,
            analysis_id,
            changes,
            overall_risk_score,
            summary,
            critical_changes_count,
            processed_at: Utc::now(),
        })
    }

    /// Process AI chat message and generate response
    pub async fn process_chat_message(
        &self,
        user_id: Uuid,
        request: AiChatRequest,
    ) -> Result<AiChatResponse, anyhow::Error> {
        let session_id = request.session_id.unwrap_or_else(Uuid::new_v4);

        // Save user message
        let user_message_id = Uuid::new_v4();
        self.save_chat_message(AiChatMessage {
            id: user_message_id,
            session_id,
            document_id: request.document_id,
            user_id,
            role: "user".to_string(),
            content: request.message.clone(),
            context: request.context.clone(),
            created_at: Utc::now(),
        })
        .await?;

        // Get conversation context
        let conversation_history = self.get_conversation_history(session_id).await?;

        // Get document context if provided
        let document_context = if let Some(doc_id) = request.document_id {
            Some(self.get_document_analysis_context(doc_id).await?)
        } else {
            None
        };

        // Generate AI response
        let ai_response = self
            .generate_ai_response(
                &request.message,
                &conversation_history,
                document_context.as_ref(),
            )
            .await?;

        // Save AI response
        let response_message_id = Uuid::new_v4();
        self.save_chat_message(AiChatMessage {
            id: response_message_id,
            session_id,
            document_id: request.document_id,
            user_id,
            role: "assistant".to_string(),
            content: ai_response.response.clone(),
            context: None,
            created_at: Utc::now(),
        })
        .await?;

        Ok(ai_response)
    }

    /// Generate negotiation insights for a document
    pub async fn generate_negotiation_insights(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<NegotiationInsight>, anyhow::Error> {
        // Get document content and recent analyses
        let document_content = self.get_document_content(document_id).await?;
        let recent_analyses = self.get_recent_analyses(document_id).await?;

        // Generate insights using AI
        let insights = self
            .generate_ai_insights(&document_content, &recent_analyses)
            .await?;

        // Save insights to database
        for insight in &insights {
            self.save_negotiation_insight(insight).await?;
        }

        Ok(insights)
    }

    /// Generate insights using vector search with a custom prompt (BAML format)
    ///
    /// This method calls the Modal-hosted agent which handles:
    /// 1. Embedding generation
    /// 2. SurrealDB vector search
    /// 3. LLM inference
    /// 4. Structured BAML response generation
    pub async fn generate_insights_with_prompt(
        &self,
        document_id: Uuid,
        request: InsightsRequest,
    ) -> Result<Vec<BAMLInsightResponse>, anyhow::Error> {
        // Call Modal-hosted agent
        let modal_response = self.call_modal_agent(document_id, &request).await?;

        Ok(modal_response)
    }

    /// Call Modal-hosted i2p-agent for insight generation
    async fn call_modal_agent(
        &self,
        document_id: Uuid,
        request: &InsightsRequest,
    ) -> Result<Vec<BAMLInsightResponse>, anyhow::Error> {
        // Get Modal endpoint from environment
        let modal_endpoint = std::env::var("MODAL_AGENT_ENDPOINT").unwrap_or_else(|_| {
            "https://ardaglobal--i2p-agent-generate-insights.modal.run".to_string()
        });

        let modal_api_key = std::env::var("MODAL_API_KEY").ok();

        // Create HTTP client
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        // Build request payload for Modal agent
        let payload = ModalInsightsRequest {
            document_id: document_id.to_string(),
            prompt: request.prompt.clone(),
            company_id: request.company_id.map(|id| id.to_string()),
            max_chunks: request.max_chunks.unwrap_or(10),
        };

        // Make request to Modal
        let mut req_builder = client
            .post(&modal_endpoint)
            .header("Content-Type", "application/json")
            .json(&payload);

        // Add API key if available
        if let Some(api_key) = modal_api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req_builder
            .send()
            .await
            .context("Failed to send request to Modal agent")?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Modal agent error ({}): {}", status, error_text);
        }

        // Parse Modal response
        let modal_response: ModalInsightsResponse = response
            .json()
            .await
            .context("Failed to parse Modal agent response")?;

        // Convert Modal response to our BAML format
        let insights = modal_response
            .insights
            .into_iter()
            .map(|insight| BAMLInsightResponse {
                insight_id: Uuid::new_v4(),
                document_id,
                insight_type: insight.insight_type,
                title: insight.title,
                description: insight.description,
                supporting_evidence: insight
                    .supporting_evidence
                    .into_iter()
                    .map(|e| EvidenceChunk {
                        text: e.text,
                        page_number: e.page_number,
                        relevance_score: e.relevance_score,
                    })
                    .collect(),
                recommended_actions: insight.recommended_actions,
                confidence: insight.confidence,
                impact: parse_impact_level(&insight.impact),
                generated_at: Utc::now(),
            })
            .collect();

        Ok(insights)
    }

    // Private helper methods

    async fn get_document_content(&self, document_id: Uuid) -> Result<String, anyhow::Error> {
        // TODO: Implement S3 content retrieval using s3_key from documents table
        // For now, return placeholder content
        let _record = sqlx::query!(
            "SELECT s3_key, filename FROM documents WHERE id = $1",
            document_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok("Document content would be fetched from S3 here".to_string())
    }

    async fn perform_ai_diff_analysis(
        &self,
        current_content: &str,
        previous_content: Option<&String>,
        focus_areas: &Option<Vec<ChangeCategory>>,
    ) -> Result<Vec<ChangeAnalysis>, anyhow::Error> {
        // This would integrate with your preferred LLM API (OpenAI, Anthropic, etc.)
        // For now, implementing a mock analysis

        let mut changes = Vec::new();

        // Simulate AI analysis results
        if let Some(prev_content) = previous_content {
            // Detect changes between versions
            changes.extend(
                self.detect_financial_changes(current_content, prev_content)
                    .await?,
            );
            changes.extend(
                self.detect_legal_changes(current_content, prev_content)
                    .await?,
            );
            changes.extend(
                self.detect_operational_changes(current_content, prev_content)
                    .await?,
            );
        } else {
            // First version analysis
            changes.extend(self.analyze_initial_document(current_content).await?);
        }

        // Filter by focus areas if specified
        if let Some(areas) = focus_areas {
            changes.retain(|c| areas.contains(&c.category));
        }

        Ok(changes)
    }

    async fn detect_financial_changes(
        &self,
        current: &str,
        previous: &str,
    ) -> Result<Vec<ChangeAnalysis>, anyhow::Error> {
        // Mock implementation - would use LLM for real analysis
        let mut changes = Vec::new();

        // Example: Interest rate change detection
        if current.contains("interest rate") && current != previous {
            changes.push(ChangeAnalysis {
                id: Uuid::new_v4(),
                document_id: Uuid::new_v4(), // Would be passed as parameter
                change_type: ChangeType::Modification,
                impact_level: ImpactLevel::Significant,
                category: ChangeCategory::Financial,
                summary: "Interest rate modified in loan agreement".to_string(),
                implications: vec![
                    "Affects total cost of borrowing".to_string(),
                    "May impact loan affordability calculations".to_string(),
                ],
                recommendations: vec![
                    "Review impact on cash flow projections".to_string(),
                    "Consider negotiating rate caps or floors".to_string(),
                ],
                related_clauses: vec!["Section 3.1: Interest and Fees".to_string()],
                risk_factors: vec![RiskFactor {
                    risk_type: "Financial Risk".to_string(),
                    severity: ImpactLevel::Moderate,
                    description: "Increased borrowing costs".to_string(),
                    mitigation_suggestions: vec![
                        "Negotiate better terms".to_string(),
                        "Lock in current rate".to_string(),
                    ],
                }],
                confidence_score: 0.85,
                created_at: Utc::now(),
            });
        }

        Ok(changes)
    }

    async fn detect_legal_changes(
        &self,
        current: &str,
        previous: &str,
    ) -> Result<Vec<ChangeAnalysis>, anyhow::Error> {
        // Mock implementation for legal changes
        let mut changes = Vec::new();

        if current.contains("guarantee") && !previous.contains("guarantee") {
            changes.push(ChangeAnalysis {
                id: Uuid::new_v4(),
                document_id: Uuid::new_v4(),
                change_type: ChangeType::Addition,
                impact_level: ImpactLevel::Critical,
                category: ChangeCategory::Legal,
                summary: "Personal guarantee clause added".to_string(),
                implications: vec![
                    "Personal assets now at risk".to_string(),
                    "Significantly increases guarantor liability".to_string(),
                ],
                recommendations: vec![
                    "Consider limiting guarantee scope".to_string(),
                    "Negotiate guarantee caps".to_string(),
                    "Review with legal counsel".to_string(),
                ],
                related_clauses: vec!["Section 8: Guarantees and Security".to_string()],
                risk_factors: vec![RiskFactor {
                    risk_type: "Legal Risk".to_string(),
                    severity: ImpactLevel::Critical,
                    description: "Unlimited personal guarantee exposure".to_string(),
                    mitigation_suggestions: vec![
                        "Limit guarantee amount".to_string(),
                        "Add guarantee release conditions".to_string(),
                    ],
                }],
                confidence_score: 0.92,
                created_at: Utc::now(),
            });
        }

        Ok(changes)
    }

    async fn detect_operational_changes(
        &self,
        _current: &str,
        _previous: &str,
    ) -> Result<Vec<ChangeAnalysis>, anyhow::Error> {
        // Mock implementation for operational changes
        Ok(vec![])
    }

    async fn analyze_initial_document(
        &self,
        _content: &str,
    ) -> Result<Vec<ChangeAnalysis>, anyhow::Error> {
        // Mock implementation for initial document analysis
        Ok(vec![])
    }

    fn calculate_overall_risk_score(&self, changes: &[ChangeAnalysis]) -> f64 {
        let mut total_score = 0.0;
        let mut count = 0;

        for change in changes {
            let impact_weight = match change.impact_level {
                ImpactLevel::Critical => 1.0,
                ImpactLevel::Significant => 0.7,
                ImpactLevel::Moderate => 0.4,
                ImpactLevel::Minor => 0.1,
            };

            total_score += change.confidence_score * impact_weight;
            count += 1;
        }

        if count > 0 {
            total_score / count as f64
        } else {
            0.0
        }
    }

    async fn generate_analysis_summary(
        &self,
        changes: &[ChangeAnalysis],
    ) -> Result<String, anyhow::Error> {
        let critical_count = changes
            .iter()
            .filter(|c| c.impact_level == ImpactLevel::Critical)
            .count();
        let significant_count = changes
            .iter()
            .filter(|c| c.impact_level == ImpactLevel::Significant)
            .count();

        let summary = if critical_count > 0 {
            format!(
                "Document contains {} critical changes requiring immediate attention and {} significant modifications. Recommend thorough legal review before proceeding.",
                critical_count, significant_count
            )
        } else if significant_count > 0 {
            format!(
                "Document has {} significant changes that may impact deal terms. Review recommended before approval.",
                significant_count
            )
        } else {
            "Document changes are primarily minor modifications with low impact on overall deal structure.".to_string()
        };

        Ok(summary)
    }

    async fn save_analysis_results(
        &self,
        analysis_id: Uuid,
        request: &DocumentDiffRequest,
        changes: &[ChangeAnalysis],
    ) -> Result<(), anyhow::Error> {
        // Save the main analysis record
        sqlx::query!(
            r#"
            INSERT INTO ai_document_analyses (
                id, document_id, analysis_type, total_changes,
                critical_changes_count, overall_risk_score, summary, status, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            "#,
            analysis_id,
            request.document_id,
            request.analysis_type.as_deref().unwrap_or("full_diff"),
            changes.len() as i32,
            0i32, // critical_changes_count - will calculate later
            Decimal::try_from(0.0f64).unwrap_or_default(), // overall_risk_score - will calculate later
            "AI document analysis completed",              // summary
            "completed"                                    // status
        )
        .execute(&self.pool)
        .await?;

        // Save individual change analyses
        for change in changes {
            self.save_change_analysis(analysis_id, change).await?;
        }

        Ok(())
    }

    async fn save_change_analysis(
        &self,
        analysis_id: Uuid,
        change: &ChangeAnalysis,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"
            INSERT INTO ai_change_analyses (
                id, analysis_id, document_id, change_type, impact_level,
                category, summary, implications, recommendations,
                related_clauses, risk_factors, confidence_score, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW())
            "#,
            change.id,
            analysis_id,
            change.document_id,
            serde_json::to_string(&change.change_type)?,
            serde_json::to_string(&change.impact_level)?,
            serde_json::to_string(&change.category)?,
            change.summary,
            serde_json::to_value(&change.implications)?,
            serde_json::to_value(&change.recommendations)?,
            serde_json::to_value(&change.related_clauses)?,
            serde_json::to_value(&change.risk_factors)?,
            Decimal::try_from(change.confidence_score).unwrap_or_default()
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn save_chat_message(&self, message: AiChatMessage) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"
            INSERT INTO ai_chat_messages (
                id, session_id, document_id, user_id, role, content, context, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            message.id,
            message.session_id,
            message.document_id,
            message.user_id,
            message.role,
            message.content,
            message.context,
            message.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_conversation_history(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<AiChatMessage>, anyhow::Error> {
        let records = sqlx::query!(
            r#"
            SELECT id, session_id, document_id, user_id, role, content, context, created_at
            FROM ai_chat_messages
            WHERE session_id = $1
            ORDER BY created_at ASC
            LIMIT 50
            "#,
            session_id
        )
        .fetch_all(&self.pool)
        .await?;

        let messages = records
            .into_iter()
            .map(|record| AiChatMessage {
                id: record.id,
                session_id: record.session_id,
                document_id: record.document_id,
                user_id: record.user_id,
                role: record.role,
                content: record.content,
                context: record.context,
                created_at: record.created_at.unwrap_or_else(|| chrono::Utc::now()),
            })
            .collect();

        Ok(messages)
    }

    async fn get_document_analysis_context(
        &self,
        document_id: Uuid,
    ) -> Result<serde_json::Value, anyhow::Error> {
        // Get recent analyses for this document
        let analyses = self.get_recent_analyses(document_id).await?;

        Ok(serde_json::json!({
            "document_id": document_id,
            "recent_analyses": analyses,
            "analysis_count": analyses.len()
        }))
    }

    async fn get_recent_analyses(
        &self,
        document_id: Uuid,
    ) -> Result<Vec<ChangeAnalysis>, anyhow::Error> {
        let records = sqlx::query!(
            r#"
            SELECT ca.id, ca.document_id, ca.change_type, ca.impact_level, ca.category,
                   ca.summary, ca.implications, ca.recommendations, ca.related_clauses,
                   ca.risk_factors, ca.confidence_score, ca.created_at
            FROM ai_change_analyses ca
            JOIN ai_document_analyses da ON ca.analysis_id = da.id
            WHERE da.document_id = $1
            ORDER BY ca.created_at DESC
            LIMIT 10
            "#,
            document_id
        )
        .fetch_all(&self.pool)
        .await?;

        let analyses = records
            .into_iter()
            .map(|record| ChangeAnalysis {
                id: record.id,
                document_id: record.document_id,
                change_type: serde_json::from_str(&record.change_type)
                    .unwrap_or(ChangeType::Modification),
                impact_level: serde_json::from_str(&record.impact_level)
                    .unwrap_or(ImpactLevel::Minor),
                category: serde_json::from_str(&record.category)
                    .unwrap_or(ChangeCategory::Operational),
                summary: record.summary,
                implications: serde_json::from_value(record.implications.unwrap_or_default())
                    .unwrap_or_default(),
                recommendations: serde_json::from_value(record.recommendations.unwrap_or_default())
                    .unwrap_or_default(),
                related_clauses: serde_json::from_value(record.related_clauses.unwrap_or_default())
                    .unwrap_or_default(),
                risk_factors: serde_json::from_value(record.risk_factors.unwrap_or_default())
                    .unwrap_or_default(),
                confidence_score: record.confidence_score.to_f64().unwrap_or(0.0),
                created_at: record.created_at.unwrap_or_else(|| chrono::Utc::now()),
            })
            .collect();

        Ok(analyses)
    }

    async fn generate_ai_response(
        &self,
        message: &str,
        conversation_history: &[AiChatMessage],
        _document_context: Option<&serde_json::Value>,
    ) -> Result<AiChatResponse, anyhow::Error> {
        // Mock AI response generation
        // In production, this would call your chosen LLM API

        let response = if message.to_lowercase().contains("interest rate") {
            "I can see you're asking about interest rate changes. Based on the document analysis, the interest rate was modified from 8.5% to 9.25%, representing a 0.75% increase. This change falls into the 'Significant' impact category as it affects the total cost of borrowing. Would you like me to explain the financial implications in more detail?"
        } else if message.to_lowercase().contains("guarantee") {
            "The guarantee clause is a critical addition to this document. It introduces personal liability for the guarantors, which significantly increases their risk exposure. I recommend reviewing the guarantee terms carefully and considering whether limits or caps should be negotiated."
        } else {
            "I understand your question. Let me analyze the relevant document sections and provide insights based on the current analysis. Could you provide more specific details about which aspect of the document you'd like me to focus on?"
        };

        Ok(AiChatResponse {
            session_id: conversation_history
                .first()
                .map(|m| m.session_id)
                .unwrap_or_else(Uuid::new_v4),
            message_id: Uuid::new_v4(),
            response: response.to_string(),
            suggestions: vec![
                "Explain financial implications".to_string(),
                "Show risk assessment".to_string(),
                "Compare to market standards".to_string(),
            ],
            related_analyses: vec![],
            confidence_score: 0.8,
        })
    }

    async fn generate_ai_insights(
        &self,
        _document_content: &str,
        _recent_analyses: &[ChangeAnalysis],
    ) -> Result<Vec<NegotiationInsight>, anyhow::Error> {
        // Mock insight generation
        Ok(vec![NegotiationInsight {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            insight_type: "market_comparison".to_string(),
            title: "Interest Rate Above Market Average".to_string(),
            description: "The proposed interest rate of 9.25% is 1.5% above the current market average for similar loan products.".to_string(),
            market_comparison: Some("Market average: 7.75%, Your rate: 9.25%".to_string()),
            historical_data: None,
            recommended_action: Some("Negotiate for a rate closer to market average or request rate protection clauses.".to_string()),
            priority: ImpactLevel::Significant,
            created_at: Utc::now(),
        }])
    }

    async fn save_negotiation_insight(
        &self,
        insight: &NegotiationInsight,
    ) -> Result<(), anyhow::Error> {
        sqlx::query!(
            r#"
            INSERT INTO ai_negotiation_insights (
                id, document_id, insight_type, title, description,
                market_comparison, historical_data, recommended_action,
                priority, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            insight.id,
            insight.document_id,
            insight.insight_type,
            insight.title,
            insight.description,
            insight.market_comparison,
            insight.historical_data,
            insight.recommended_action,
            serde_json::to_string(&insight.priority)?,
            insight.created_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_analysis_serialization() {
        let change = ChangeAnalysis {
            id: Uuid::new_v4(),
            document_id: Uuid::new_v4(),
            change_type: ChangeType::Modification,
            impact_level: ImpactLevel::Significant,
            category: ChangeCategory::Financial,
            summary: "Test change".to_string(),
            implications: vec!["Implication 1".to_string()],
            recommendations: vec!["Recommendation 1".to_string()],
            related_clauses: vec!["Clause 1".to_string()],
            risk_factors: vec![],
            confidence_score: 0.85,
            created_at: Utc::now(),
        };

        let serialized = serde_json::to_string(&change).unwrap();
        let deserialized: ChangeAnalysis = serde_json::from_str(&serialized).unwrap();

        assert_eq!(change.change_type, deserialized.change_type);
        assert_eq!(change.impact_level, deserialized.impact_level);
        assert_eq!(change.category, deserialized.category);
    }

    #[tokio::test]
    async fn test_risk_calculation() {
        let database_url = "postgresql://test:test@localhost:5432/test_db";
        let pool = match PgPool::connect(database_url).await {
            Ok(p) => p,
            Err(_) => {
                // Skip test if database not available
                return;
            }
        };
        let service = AiAnalysisService::new(pool);

        let changes = vec![
            ChangeAnalysis {
                id: Uuid::new_v4(),
                document_id: Uuid::new_v4(),
                change_type: ChangeType::Addition,
                impact_level: ImpactLevel::Critical,
                category: ChangeCategory::Legal,
                summary: "Critical change".to_string(),
                implications: vec![],
                recommendations: vec![],
                related_clauses: vec![],
                risk_factors: vec![],
                confidence_score: 0.9,
                created_at: Utc::now(),
            },
            ChangeAnalysis {
                id: Uuid::new_v4(),
                document_id: Uuid::new_v4(),
                change_type: ChangeType::Modification,
                impact_level: ImpactLevel::Minor,
                category: ChangeCategory::Operational,
                summary: "Minor change".to_string(),
                implications: vec![],
                recommendations: vec![],
                related_clauses: vec![],
                risk_factors: vec![],
                confidence_score: 0.5,
                created_at: Utc::now(),
            },
        ];

        let risk_score = service.calculate_overall_risk_score(&changes);

        // Should be weighted towards the critical change
        assert!(risk_score > 0.5);
        assert!(risk_score < 1.0);
    }
}
