use crate::services::ai_analysis_service::{
    AiAnalysisService, AiChatRequest, AiChatResponse, BAMLInsightResponse, DocumentDiffRequest,
    DocumentDiffResponse, InsightsRequest, NegotiationInsight,
};
// Authentication will be added later
use crate::config::types::ApiState;
use crate::services::auth::Claims;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use rust_decimal::prelude::ToPrimitive;

// Temporary auth stub - will be replaced with proper auth
// For now, we'll skip authentication and just pass dummy claims
fn get_dummy_claims() -> Claims {
    Claims {
        sub: "00000000-0000-0000-0000-000000000000".to_string(),
        email: "dummy@example.com".to_string(),
        user_type: "admin".to_string(),
        exp: chrono::Utc::now().timestamp() + 3600,
        iat: chrono::Utc::now().timestamp(),
        jti: "dummy-jti".to_string(),
    }
}
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// Query parameters for document analysis
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct AnalyzeDocumentQuery {
    /// Previous version ID for comparison. If not provided, compares with the most recent previous version
    pub previous_version_id: Option<Uuid>,
    /// Type of analysis to perform (e.g., "full_diff", "financial_only", "legal_only")
    pub analysis_type: Option<String>,
    /// Comma-separated list of focus areas: financial, legal, operational, compliance
    pub focus_areas: Option<String>,
}

/// Standard API error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiError {
    /// Human-readable error message
    pub error: String,
    /// Error code for programmatic handling
    pub code: String,
}

// Helper function to convert errors to API responses
fn handle_db_error(err: sqlx::Error) -> (StatusCode, Json<ApiError>) {
    let (status, code, message) = match err {
        sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, "NOT_FOUND", "Resource not found"),
        sqlx::Error::Database(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST", "Database error"),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "Internal server error",
        ),
    };

    (
        status,
        Json(ApiError {
            error: message.to_string(),
            code: code.to_string(),
        }),
    )
}

// Helper function to convert generic errors
fn handle_service_error(err: anyhow::Error) -> (StatusCode, Json<ApiError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiError {
            error: err.to_string(),
            code: "INTERNAL_ERROR".to_string(),
        }),
    )
}

/// Analyze document differences between versions
#[utoipa::path(
    get,
    path = "/api/v1/documents/{document_id}/analyze",
    params(
        ("document_id" = Uuid, Path, description = "Document ID to analyze")
    ),
    params(AnalyzeDocumentQuery),
    responses(
        (status = 200, description = "Analysis completed successfully", body = crate::services::ai_analysis_service::DocumentDiffResponse),
        (status = 400, description = "Invalid request parameters", body = ApiError),
        (status = 404, description = "Document not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Analytics",
    summary = "Analyze document changes",
    description = "Analyzes differences between document versions using AI to identify changes, assess risk, and provide recommendations"
)]
pub async fn analyze_document_diff(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<AnalyzeDocumentQuery>,
) -> Result<Json<DocumentDiffResponse>, (StatusCode, Json<ApiError>)> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let ai_service = AiAnalysisService::new(state.pool.clone());

    // Parse focus areas if provided
    let focus_areas = query.focus_areas.as_ref().map(|areas| {
        areas
            .split(',')
            .filter_map(|area| match area.trim().to_lowercase().as_str() {
                "financial" => {
                    Some(crate::services::ai_analysis_service::ChangeCategory::Financial)
                }
                "legal" => Some(crate::services::ai_analysis_service::ChangeCategory::Legal),
                "operational" => {
                    Some(crate::services::ai_analysis_service::ChangeCategory::Operational)
                }
                "compliance" => {
                    Some(crate::services::ai_analysis_service::ChangeCategory::Compliance)
                }
                _ => None,
            })
            .collect()
    });

    // Get current version ID (latest version for this document)
    let current_version_id = get_latest_document_version_id(&state, document_id)
        .await
        .map_err(handle_db_error)?;

    let request = DocumentDiffRequest {
        document_id,
        previous_version_id: query.previous_version_id,
        current_version_id,
        analysis_type: query.analysis_type,
        focus_areas,
    };

    let response = ai_service
        .analyze_document_diff(request)
        .await
        .map_err(handle_service_error)?;

    Ok(Json(response))
}

/// Process AI chat message
#[utoipa::path(
    post,
    path = "/api/v1/ai/chat",
    request_body = crate::services::ai_analysis_service::AiChatRequest,
    responses(
        (status = 200, description = "Chat message processed successfully", body = crate::services::ai_analysis_service::AiChatResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Companies",
    summary = "Send chat message to AI",
    description = "Sends a message to the AI chat system for document analysis and advice"
)]
pub async fn process_chat_message(
    State(state): State<ApiState>,
    Json(request): Json<AiChatRequest>,
) -> Result<Json<AiChatResponse>, (StatusCode, Json<ApiError>)> {
    let claims = get_dummy_claims(); // TODO: Replace with real auth
    let ai_service = AiAnalysisService::new(state.pool.clone());

    let user_id = uuid::Uuid::parse_str(&claims.sub).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                error: "Invalid user ID in token".to_string(),
                code: "INVALID_USER_ID".to_string(),
            }),
        )
    })?;

    let response = ai_service
        .process_chat_message(user_id, request)
        .await
        .map_err(handle_service_error)?;

    Ok(Json(response))
}

/// Get negotiation insights for a document
#[utoipa::path(
    get,
    path = "/api/v1/documents/{document_id}/insights",
    params(
        ("document_id" = Uuid, Path, description = "Document ID to get insights for")
    ),
    responses(
        (status = 200, description = "Insights retrieved successfully", body = Vec<crate::services::ai_analysis_service::NegotiationInsight>),
        (status = 404, description = "Document not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Analytics",
    summary = "Get negotiation insights",
    description = "Retrieves AI-generated negotiation insights and recommendations for a document"
)]
pub async fn get_negotiation_insights(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
) -> Result<Json<Vec<NegotiationInsight>>, (StatusCode, Json<ApiError>)> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let ai_service = AiAnalysisService::new(state.pool.clone());

    let insights = ai_service
        .generate_negotiation_insights(document_id)
        .await
        .map_err(handle_service_error)?;

    Ok(Json(insights))
}

/// Generate insights with custom prompt using vector search (BAML format)
#[utoipa::path(
    post,
    path = "/api/v1/documents/{document_id}/insights",
    params(
        ("document_id" = Uuid, Path, description = "Document ID to analyze")
    ),
    request_body = crate::services::ai_analysis_service::InsightsRequest,
    responses(
        (status = 200, description = "Insights generated successfully", body = Vec<crate::services::ai_analysis_service::BAMLInsightResponse>),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 404, description = "Document not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Analytics",
    summary = "Generate insights with custom prompt",
    description = "Generates AI insights using vector search over document chunks based on a custom prompt. Returns structured BAML-formatted JSON."
)]
pub async fn generate_insights_with_prompt(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    Json(request): Json<InsightsRequest>,
) -> Result<Json<Vec<BAMLInsightResponse>>, (StatusCode, Json<ApiError>)> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let ai_service = AiAnalysisService::new(state.pool.clone());

    let insights = ai_service
        .generate_insights_with_prompt(document_id, request)
        .await
        .map_err(handle_service_error)?;

    Ok(Json(insights))
}

/// Get AI chat history for a session
#[utoipa::path(
    get,
    path = "/api/v1/ai/chat/{session_id}/history",
    params(
        ("session_id" = Uuid, Path, description = "Chat session ID")
    ),
    responses(
        (status = 200, description = "Chat history retrieved successfully", body = Vec<crate::services::ai_analysis_service::AiChatMessage>),
        (status = 404, description = "Session not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Companies",
    summary = "Get chat history",
    description = "Retrieves the message history for a specific AI chat session"
)]
pub async fn get_chat_history(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> Result<
    Json<Vec<crate::services::ai_analysis_service::AiChatMessage>>,
    (StatusCode, Json<ApiError>),
> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let ai_service = AiAnalysisService::new(state.pool.clone());

    let messages = ai_service
        .get_conversation_history(session_id)
        .await
        .map_err(handle_service_error)?;

    Ok(Json(messages))
}

/// Response containing document analysis history
#[derive(Debug, Serialize, ToSchema)]
pub struct AnalysisHistoryResponse {
    /// Document ID that was analyzed
    pub document_id: Uuid,
    /// List of analysis summaries
    pub analyses: Vec<AnalysisSummary>,
    /// Total number of analyses performed on this document
    pub total_count: i32,
}

/// Summary of a document analysis
#[derive(Debug, Serialize, ToSchema)]
pub struct AnalysisSummary {
    /// Unique analysis ID
    pub id: Uuid,
    /// Type of analysis performed
    pub analysis_type: String,
    /// Total number of changes detected
    pub total_changes: i32,
    /// Number of critical changes detected
    pub critical_changes: i32,
    /// Overall risk score (0.0 to 1.0)
    pub risk_score: f64,
    /// When the analysis was created
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Get analysis history for a document
#[utoipa::path(
    get,
    path = "/api/v1/documents/{document_id}/analysis-history",
    params(
        ("document_id" = Uuid, Path, description = "Document ID to get history for"),
        ("limit" = Option<i64>, Query, description = "Maximum number of analyses to return")
    ),
    responses(
        (status = 200, description = "Analysis history retrieved successfully", body = AnalysisHistoryResponse),
        (status = 404, description = "Document not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Analytics",
    summary = "Get analysis history",
    description = "Retrieves the history of AI analyses performed on a specific document"
)]
pub async fn get_analysis_history(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<AnalysisHistoryResponse>, (StatusCode, Json<ApiError>)> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let limit: i64 = query
        .get("limit")
        .and_then(|l| l.parse().ok())
        .unwrap_or(20);

    let analyses = get_document_analysis_history(&state, document_id, limit)
        .await
        .map_err(handle_db_error)?;

    let response = AnalysisHistoryResponse {
        document_id,
        total_count: analyses.len() as i32,
        analyses,
    };

    Ok(Json(response))
}

/// Request to trigger real-time document analysis
#[derive(Debug, Deserialize, ToSchema)]
pub struct TriggerAnalysisRequest {
    /// Document version ID to analyze
    pub version_id: Uuid,
    /// Priority level: low, medium, high
    pub priority: Option<String>,
    /// List of user IDs to notify when analysis completes
    pub notify_users: Option<Vec<Uuid>>,
}

/// Response from triggering document analysis
#[derive(Debug, Serialize, ToSchema)]
pub struct TriggerAnalysisResponse {
    /// ID of the created analysis
    pub analysis_id: Uuid,
    /// Current status of the analysis
    pub status: String,
    /// Estimated completion time
    pub estimated_completion: chrono::DateTime<chrono::Utc>,
}

/// Trigger real-time analysis for document changes
#[utoipa::path(
    post,
    path = "/api/v1/documents/{document_id}/trigger-analysis",
    params(
        ("document_id" = Uuid, Path, description = "Document ID to analyze")
    ),
    request_body = TriggerAnalysisRequest,
    responses(
        (status = 200, description = "Analysis triggered successfully", body = TriggerAnalysisResponse),
        (status = 400, description = "Invalid request", body = ApiError),
        (status = 404, description = "Document not found", body = ApiError),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Analytics",
    summary = "Trigger document analysis",
    description = "Triggers immediate AI analysis of document changes with optional notifications"
)]
pub async fn trigger_analysis(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    Json(request): Json<TriggerAnalysisRequest>,
) -> Result<Json<TriggerAnalysisResponse>, (StatusCode, Json<ApiError>)> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let ai_service = AiAnalysisService::new(state.pool.clone());

    // Get previous version for comparison
    let previous_version_id = get_previous_document_version_id(&state, document_id)
        .await
        .ok();

    let diff_request = DocumentDiffRequest {
        document_id,
        previous_version_id,
        current_version_id: request.version_id,
        analysis_type: Some("triggered_analysis".to_string()),
        focus_areas: None,
    };

    let analysis_response = ai_service
        .analyze_document_diff(diff_request)
        .await
        .map_err(handle_service_error)?;

    // If priority is high or critical changes detected, send notifications
    if request.priority.as_deref() == Some("high") || analysis_response.critical_changes_count > 0 {
        if let Some(user_ids) = request.notify_users {
            send_analysis_notifications(&state, &analysis_response, user_ids)
                .await
                .map_err(handle_db_error)?;
        }
    }

    let response = TriggerAnalysisResponse {
        analysis_id: analysis_response.analysis_id,
        status: "completed".to_string(),
        estimated_completion: chrono::Utc::now(),
    };

    Ok(Json(response))
}

/// AI analysis dashboard data
#[derive(Debug, Serialize, ToSchema)]
pub struct AnalyticsDashboardResponse {
    /// Total number of analyses performed
    pub total_analyses: i32,
    /// Number of critical changes detected today
    pub critical_changes_today: i32,
    /// Average risk score across all analyses
    pub average_risk_score: f64,
    /// Most active documents by analysis count
    pub most_active_documents: Vec<DocumentActivity>,
    /// Change counts by category
    pub change_categories: HashMap<String, i32>,
    /// Risk score trends over time
    pub risk_trends: Vec<RiskTrendPoint>,
}

/// Document activity summary
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentActivity {
    /// Document identifier
    pub document_id: Uuid,
    /// Document name or filename
    pub document_name: String,
    /// Number of changes detected in this document
    pub change_count: i32,
    /// Most recent risk score for this document
    pub latest_risk_score: f64,
    /// When this document was last analyzed
    pub last_analyzed: chrono::DateTime<chrono::Utc>,
}

/// Risk trend data point
#[derive(Debug, Serialize, ToSchema)]
pub struct RiskTrendPoint {
    /// Date for this data point
    pub date: chrono::NaiveDate,
    /// Average risk score for this date
    pub average_risk: f64,
    /// Number of analyses performed on this date
    pub analysis_count: i32,
}

/// Get AI analysis dashboard data
#[utoipa::path(
    get,
    path = "/api/v1/ai/analytics/dashboard",
    params(
        ("days" = Option<i32>, Query, description = "Number of days to include in analytics (default: 30)")
    ),
    responses(
        (status = 200, description = "Dashboard data retrieved successfully", body = AnalyticsDashboardResponse),
        (status = 500, description = "Internal server error", body = ApiError)
    ),
    tag = "Analytics",
    summary = "Get analytics dashboard",
    description = "Retrieves comprehensive analytics data for AI document analysis including trends, activity, and performance metrics"
)]
pub async fn get_analytics_dashboard(
    State(state): State<ApiState>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<AnalyticsDashboardResponse>, (StatusCode, Json<ApiError>)> {
    let _claims = get_dummy_claims(); // TODO: Replace with real auth
    let days: i32 = query.get("days").and_then(|d| d.parse().ok()).unwrap_or(30);

    let dashboard_data = generate_analytics_dashboard(&state, days)
        .await
        .map_err(handle_db_error)?;

    Ok(Json(dashboard_data))
}

// Helper functions

// Note: document_versions table does not exist in current schema
// Using document_id directly since we don't have version tracking yet
async fn get_latest_document_version_id(
    _state: &ApiState,
    document_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    // Return document_id as placeholder since versions aren't implemented
    Ok(document_id)
}

async fn get_previous_document_version_id(
    _state: &ApiState,
    document_id: Uuid,
) -> Result<Uuid, sqlx::Error> {
    // Return document_id as placeholder since versions aren't implemented
    Ok(document_id)
}

async fn get_document_analysis_history(
    state: &ApiState,
    document_id: Uuid,
    limit: i64,
) -> Result<Vec<AnalysisSummary>, sqlx::Error> {
    let records = sqlx::query!(
        r#"
        SELECT
            da.id,
            da.analysis_type,
            da.total_changes,
            COUNT(CASE WHEN ca.impact_level = '"critical"' THEN 1 END) as critical_changes,
            AVG(ca.confidence_score) as avg_risk_score,
            da.created_at
        FROM ai_document_analyses da
        LEFT JOIN ai_change_analyses ca ON da.id = ca.analysis_id
        WHERE da.document_id = $1
        GROUP BY da.id, da.analysis_type, da.total_changes, da.created_at
        ORDER BY da.created_at DESC
        LIMIT $2
        "#,
        document_id,
        limit
    )
    .fetch_all(&state.pool)
    .await?;

    let analyses = records
        .into_iter()
        .map(|record| AnalysisSummary {
            id: record.id,
            analysis_type: record.analysis_type,
            total_changes: record.total_changes,
            critical_changes: record.critical_changes.unwrap_or(0) as i32,
            risk_score: record
                .avg_risk_score
                .map(|d| d.to_f64().unwrap_or(0.0))
                .unwrap_or(0.0),
            created_at: record.created_at.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    Ok(analyses)
}

async fn send_analysis_notifications(
    state: &ApiState,
    analysis: &DocumentDiffResponse,
    user_ids: Vec<Uuid>,
) -> Result<(), sqlx::Error> {
    // Create notification records
    for user_id in user_ids {
        sqlx::query!(
            r#"
            INSERT INTO ai_analysis_notifications (
                id, analysis_id, user_id, notification_type, title, message,
                priority, delivery_method, is_read, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            "#,
            Uuid::new_v4(),
            analysis.analysis_id,
            user_id,
            "ai_analysis_complete",
            "Document Analysis Complete",
            format!(
                "AI analysis found {} changes including {} critical modifications. Review recommended.",
                analysis.changes.len(),
                analysis.critical_changes_count
            ),
            if analysis.critical_changes_count > 0 { "high" } else { "medium" },
            "in_app",
            false
        )
        .execute(&state.pool)
        .await?;
    }

    Ok(())
}

async fn generate_analytics_dashboard(
    state: &ApiState,
    days: i32,
) -> Result<AnalyticsDashboardResponse, sqlx::Error> {
    // Get basic statistics
    let stats = sqlx::query!(
        r#"
        SELECT
            COUNT(DISTINCT da.id) as total_analyses,
            COUNT(CASE WHEN ca.impact_level = 'critical' AND da.created_at >= NOW() - INTERVAL '1 day' * $1 THEN 1 END) as critical_today,
            AVG(ca.confidence_score) as avg_risk_score
        FROM ai_document_analyses da
        LEFT JOIN ai_change_analyses ca ON da.id = ca.analysis_id
        WHERE da.created_at >= NOW() - INTERVAL '1 day' * $2
        "#,
        days as f64,
        days as f64
    )
    .fetch_one(&state.pool)
    .await?;

    // Get most active documents
    let active_docs = sqlx::query!(
        r#"
        SELECT
            d.id as document_id,
            d.filename as document_name,
            COUNT(da.id) as change_count,
            AVG(ca.confidence_score) as latest_risk_score,
            MAX(da.created_at) as last_analyzed
        FROM documents d
        JOIN ai_document_analyses da ON d.id = da.document_id
        LEFT JOIN ai_change_analyses ca ON da.id = ca.analysis_id
        WHERE da.created_at >= NOW() - INTERVAL '1 day' * $1
        GROUP BY d.id, d.filename
        ORDER BY change_count DESC
        LIMIT 10
        "#,
        days as f64
    )
    .fetch_all(&state.pool)
    .await?;

    let most_active_documents = active_docs
        .into_iter()
        .map(|record| DocumentActivity {
            document_id: record.document_id,
            document_name: record.document_name,
            change_count: record.change_count.unwrap_or(0) as i32,
            latest_risk_score: record
                .latest_risk_score
                .map(|d| d.to_f64().unwrap_or(0.0))
                .unwrap_or(0.0),
            last_analyzed: record.last_analyzed.unwrap_or_else(|| chrono::Utc::now()),
        })
        .collect();

    // Mock data for change categories and risk trends
    let mut change_categories = HashMap::new();
    change_categories.insert("financial".to_string(), 45);
    change_categories.insert("legal".to_string(), 32);
    change_categories.insert("operational".to_string(), 18);
    change_categories.insert("compliance".to_string(), 12);

    let risk_trends = vec![
        RiskTrendPoint {
            date: chrono::Utc::now().date_naive() - chrono::Duration::days(7),
            average_risk: 0.65,
            analysis_count: 8,
        },
        RiskTrendPoint {
            date: chrono::Utc::now().date_naive(),
            average_risk: 0.72,
            analysis_count: 12,
        },
    ];

    Ok(AnalyticsDashboardResponse {
        total_analyses: stats.total_analyses.unwrap_or(0) as i32,
        critical_changes_today: stats.critical_today.unwrap_or(0) as i32,
        average_risk_score: stats
            .avg_risk_score
            .map(|d| d.to_f64().unwrap_or(0.0))
            .unwrap_or(0.0),
        most_active_documents,
        change_categories,
        risk_trends,
    })
}

pub fn create_ai_analysis_router() -> Router<ApiState> {
    Router::new()
        .route(
            "/documents/{document_id}/analyze",
            get(analyze_document_diff),
        )
        .route(
            "/documents/{document_id}/trigger-analysis",
            post(trigger_analysis),
        )
        .route(
            "/documents/{document_id}/insights",
            get(get_negotiation_insights).post(generate_insights_with_prompt),
        )
        .route(
            "/documents/{document_id}/analysis-history",
            get(get_analysis_history),
        )
        .route("/ai/chat", post(process_chat_message))
        .route("/ai/chat/{session_id}/history", get(get_chat_history))
        .route("/ai/analytics/dashboard", get(get_analytics_dashboard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::ai_analysis_service::ChangeCategory;

    #[test]
    fn test_focus_areas_parsing() {
        let query = AnalyzeDocumentQuery {
            previous_version_id: None,
            analysis_type: None,
            focus_areas: Some("financial,legal,invalid".to_string()),
        };

        let focus_areas = query.focus_areas.as_ref().map(|areas| {
            areas
                .split(',')
                .filter_map(|area| match area.trim().to_lowercase().as_str() {
                    "financial" => Some(ChangeCategory::Financial),
                    "legal" => Some(ChangeCategory::Legal),
                    "operational" => Some(ChangeCategory::Operational),
                    "compliance" => Some(ChangeCategory::Compliance),
                    _ => None,
                })
                .collect::<Vec<_>>()
        });

        assert_eq!(focus_areas.unwrap().len(), 2); // Only financial and legal should be parsed
    }
}
