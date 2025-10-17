//! AI-powered semantic search endpoints with multi-tenant support
//!
//! This module provides HTTP endpoints for semantic search across documents
//! with company-based partitioning for privacy and performance.

use crate::{api::router::ApiState, config::types::ErrorResponse, services::auth::Claims};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use utoipa::ToSchema;
use uuid::Uuid;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Semantic search request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SemanticSearchRequest {
    /// Search query
    #[serde(rename = "query")]
    pub query: String,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Optional document type filter
    pub document_type: Option<String>,

    /// Optional deal ID filter
    pub deal_id: Option<Uuid>,
}

fn default_limit() -> usize {
    10
}

/// Search result item
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResultItem {
    /// Chunk ID
    pub chunk_id: Uuid,

    /// Relevance score (0.0 to 1.0)
    pub score: f32,

    /// Document ID
    pub document_id: Uuid,

    /// Company ID (for verification)
    pub company_id: Uuid,

    /// Matching text chunk
    pub text: String,

    /// Page number (if applicable)
    pub page_number: Option<i32>,

    /// Document type
    pub document_type: String,

    /// Deal ID (if associated)
    pub deal_id: Option<Uuid>,
}

/// Semantic search response
#[derive(Debug, Serialize, ToSchema)]
pub struct SemanticSearchResponse {
    /// Search query
    pub query: String,

    /// Search results
    pub results: Vec<SearchResultItem>,

    /// Total results found
    pub total_results: usize,

    /// Processing time in milliseconds
    pub processing_time_ms: u128,
}

/// Document search by ID request
#[derive(Debug, Deserialize, ToSchema)]
pub struct DocumentSearchRequest {
    /// Search query within document
    pub query: String,

    /// Maximum number of results
    #[serde(default = "default_limit")]
    pub limit: usize,
}

// ============================================================================
// ERROR HANDLING
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Access denied: {0}")]
    Forbidden(String),

    #[error("Company not found: {0}")]
    CompanyNotFound(Uuid),

    #[error("Document not found: {0}")]
    DocumentNotFound(Uuid),

    #[error("Search service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl IntoResponse for SearchError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            SearchError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Authentication required".to_string(),
            ),
            SearchError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg.clone()),
            SearchError::CompanyNotFound(id) => (
                StatusCode::NOT_FOUND,
                "company_not_found",
                format!("Company {} not found", id),
            ),
            SearchError::DocumentNotFound(id) => (
                StatusCode::NOT_FOUND,
                "document_not_found",
                format!("Document {} not found", id),
            ),
            SearchError::ServiceUnavailable(msg) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                msg.clone(),
            ),
            SearchError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            SearchError::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                msg.clone(),
            ),
        };

        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message,
            details: None,
        };

        (status, Json(error_response)).into_response()
    }
}

// ============================================================================
// ENDPOINTS
// ============================================================================

/// Semantic search across company's documents
///
/// Search across all documents belonging to the authenticated user's company
/// using semantic similarity (vector search).
#[utoipa::path(
    post,
    path = "/api/companies/{company_id}/search",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    request_body = SemanticSearchRequest,
    responses(
        (status = 200, description = "Search results", body = SemanticSearchResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 503, description = "Search service unavailable"),
        (status = 500, description = "Internal server error")
    ),
    tag = "AI Search",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn semantic_search(
    State(_state): State<ApiState>,
    Path(company_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<SemanticSearchRequest>,
) -> Result<Json<SemanticSearchResponse>, SearchError> {
    let start_time = std::time::Instant::now();

    // Extract and validate auth claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        query = %request.query,
        "Semantic search request received"
    );

    // Validate company access
    validate_company_access(&claims, company_id).await?;

    // TODO: Implement via Modal agent (similar to insights endpoint)
    // For now, return empty results
    // Get services
    // let embedding_service = state.embedding_service.as_ref().ok_or_else(|| {
    //     SearchError::ServiceUnavailable("Embedding service not available".to_string())
    // })?;

    // let surrealdb_service = state.surrealdb_service.as_ref().ok_or_else(|| {
    //     SearchError::ServiceUnavailable("Vector search service not available".to_string())
    // })?;

    // // Generate query embedding
    // let query_embedding = embedding_service
    //     .embed(&request.query)
    //     .await
    //     .map_err(|e| SearchError::InternalError(format!("Failed to embed query: {}", e)))?;

    // // Search in SurrealDB with company filtering (multi-tenant)
    // let search_results = surrealdb_service
    //     .search_by_company(Some(company_id), query_embedding, request.limit)
    //     .await
    //     .map_err(|e| SearchError::InternalError(format!("Search failed: {}", e)))?;

    let search_results: Vec<crate::services::document_ingestion::SearchResult> = vec![];

    // Convert to response format
    // TODO: Update this when implementing search functionality with SurrealDB
    let results: Vec<SearchResultItem> = search_results
        .into_iter()
        .map(|result| SearchResultItem {
            chunk_id: result.embedding_id, // Updated field name
            score: result.score,
            document_id: result.document_id,
            company_id: Uuid::nil(), // TODO: Get from document metadata
            text: result.chunk_text,
            page_number: result.page_number,
            document_type: "unknown".to_string(), // TODO: Get from document metadata
            deal_id: None,                        // TODO: Get from document metadata
        })
        .collect();

    let processing_time = start_time.elapsed();

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        result_count = results.len(),
        processing_time_ms = processing_time.as_millis(),
        "Semantic search completed"
    );

    let total_results = results.len();
    Ok(Json(SemanticSearchResponse {
        query: request.query,
        results,
        total_results,
        processing_time_ms: processing_time.as_millis(),
    }))
}

/// Search within a specific document
///
/// Semantic search scoped to a single document within the company.
#[utoipa::path(
    post,
    path = "/api/companies/{company_id}/documents/{document_id}/search",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    request_body = DocumentSearchRequest,
    responses(
        (status = 200, description = "Search results", body = SemanticSearchResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Document not found"),
        (status = 503, description = "Search service unavailable"),
        (status = 500, description = "Internal server error")
    ),
    tag = "AI Search",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn search_document(
    State(_state): State<ApiState>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(request): Json<DocumentSearchRequest>,
) -> Result<Json<SemanticSearchResponse>, SearchError> {
    let start_time = std::time::Instant::now();

    // Extract and validate auth claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        query = %request.query,
        "Document search request received"
    );

    // Validate company access
    validate_company_access(&claims, company_id).await?;

    // TODO: Implement via Modal agent (similar to insights endpoint)
    // For now, return empty results
    // Get services
    // let embedding_service = state.embedding_service.as_ref().ok_or_else(|| {
    //     SearchError::ServiceUnavailable("Embedding service not available".to_string())
    // })?;

    // let surrealdb_service = state.surrealdb_service.as_ref().ok_or_else(|| {
    //     SearchError::ServiceUnavailable("Vector search service not available".to_string())
    // })?;

    // // Generate query embedding
    // let query_embedding = embedding_service
    //     .embed(&request.query)
    //     .await
    //     .map_err(|e| SearchError::InternalError(format!("Failed to embed query: {}", e)))?;

    // // Search with document filtering (multi-tenant + document-scoped)
    // let search_results = surrealdb_service
    //     .search_by_document(
    //         document_id,
    //         query_embedding,
    //         request.limit,
    //     )
    //     .await
    //     .map_err(|e| SearchError::InternalError(format!("Search failed: {}", e)))?;

    let search_results: Vec<crate::services::document_ingestion::SearchResult> = vec![];

    // Convert to response format
    // TODO: Update this when implementing search functionality with SurrealDB
    let results: Vec<SearchResultItem> = search_results
        .into_iter()
        .map(|result| SearchResultItem {
            chunk_id: result.embedding_id, // Updated field name
            score: result.score,
            document_id: result.document_id,
            company_id: Uuid::nil(), // TODO: Get from document metadata
            text: result.chunk_text,
            page_number: result.page_number,
            document_type: "unknown".to_string(), // TODO: Get from document metadata
            deal_id: None,                        // TODO: Get from document metadata
        })
        .collect();

    let processing_time = start_time.elapsed();

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        result_count = results.len(),
        processing_time_ms = processing_time.as_millis(),
        "Document search completed"
    );

    let total_results = results.len();
    Ok(Json(SemanticSearchResponse {
        query: request.query,
        results,
        total_results,
        processing_time_ms: processing_time.as_millis(),
    }))
}

/// Health check for AI search services
#[utoipa::path(
    get,
    path = "/api/search/health",
    responses(
        (status = 200, description = "Service healthy"),
        (status = 503, description = "Service unhealthy")
    ),
    tag = "AI Search"
)]
pub async fn search_health_check(
    State(_state): State<ApiState>,
) -> Result<Json<serde_json::Value>, SearchError> {
    // TODO: Implement health check via Modal agent
    // Check embedding service
    // let embedding_healthy = if let Some(service) = state.embedding_service.as_ref() {
    //     service
    //         .health_check()
    //         .await
    //         .map_err(|e| SearchError::InternalError(format!("Embedding health check failed: {}", e)))?
    // } else {
    //     false
    // };

    // // Check SurrealDB service
    // let surrealdb_healthy = if let Some(service) = state.surrealdb_service.as_ref() {
    //     service
    //         .health_check()
    //         .await
    //         .map_err(|e| SearchError::InternalError(format!("SurrealDB health check failed: {}", e)))?
    // } else {
    //     false
    // };

    let embedding_healthy = true; // Placeholder
    let surrealdb_healthy = true; // Placeholder
    let healthy = embedding_healthy && surrealdb_healthy;
    let status = if healthy { "healthy" } else { "unhealthy" };

    if !healthy {
        return Err(SearchError::ServiceUnavailable(
            "One or more search services are unavailable".to_string(),
        ));
    }

    Ok(Json(json!({
        "status": status,
        "embedding_service": embedding_healthy,
        "vector_database": surrealdb_healthy,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Extract JWT claims from Authorization header
fn extract_claims(headers: &HeaderMap) -> Result<Claims, SearchError> {
    let auth_header = headers
        .get("Authorization")
        .ok_or(SearchError::Unauthorized)?;

    let auth_str = auth_header
        .to_str()
        .map_err(|_| SearchError::Unauthorized)?;

    let _token = auth_str
        .strip_prefix("Bearer ")
        .ok_or(SearchError::Unauthorized)?;

    // TODO: Decode and validate JWT
    // For now, return placeholder claims
    Ok(Claims {
        sub: "placeholder-user-id".to_string(),
        email: "placeholder@example.com".to_string(),
        user_type: "investor".to_string(),
        exp: chrono::Utc::now().timestamp() + 3600,
        iat: chrono::Utc::now().timestamp(),
        jti: uuid::Uuid::new_v4().to_string(),
    })
}

/// Validate user has access to company
async fn validate_company_access(claims: &Claims, company_id: Uuid) -> Result<(), SearchError> {
    // TODO: Implement actual company access validation
    // - Check if user is member of company
    // - Check if user has required permissions
    // - Verify KYC/KYB status

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        "Company access validated (placeholder)"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_request_deserialization() {
        let json = r#"{
            "query": "credit agreement",
            "limit": 5
        }"#;

        let request: SemanticSearchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.query, "credit agreement");
        assert_eq!(request.limit, 5);
    }

    #[test]
    fn test_search_response_serialization() {
        let response = SemanticSearchResponse {
            query: "test query".to_string(),
            results: vec![],
            total_results: 0,
            processing_time_ms: 100,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("test query"));
        assert!(json.contains("\"total_results\":0"));
    }
}
