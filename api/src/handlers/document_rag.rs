///! Document RAG (Retrieval-Augmented Generation) API Handlers
///!
///! Provides endpoints for AI-powered document search and question answering
///! using vector similarity search and LLM inference.
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, Json, Sse},
};
use futures_util::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tracing::{error, info};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::router::ApiState;
use crate::config::types::ErrorResponse;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Vector search request
#[derive(Debug, Deserialize, ToSchema)]
pub struct VectorSearchRequest {
    /// The search query text
    pub query: String,
    /// Number of results to return (default: 5, max: 20)
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    /// Optional company filter
    pub company_id: Option<Uuid>,
    /// Optional document filter
    pub document_id: Option<Uuid>,
}

fn default_top_k() -> usize {
    5
}

/// Vector search response
#[derive(Debug, Serialize, ToSchema)]
pub struct VectorSearchResponse {
    /// Search results with relevance scores
    pub results: Vec<SearchResultDto>,
    /// Number of results returned
    pub count: usize,
}

/// Search result DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResultDto {
    /// Document ID
    pub document_id: Uuid,
    /// Company ID
    pub company_id: Uuid,
    /// Deal ID if applicable
    pub deal_id: Option<Uuid>,
    /// Relevance score (0.0 to 1.0, higher is better)
    pub score: f32,
    /// The relevant text chunk
    pub text: String,
    /// Chunk index in the document
    pub chunk_index: usize,
    /// Position in the document
    pub chunk_position: usize,
    /// Page number if applicable
    pub page_number: Option<i32>,
}

/// RAG query request
#[derive(Debug, Deserialize, ToSchema)]
pub struct RagQueryRequest {
    /// The question to ask
    pub question: String,
    /// Number of relevant chunks to retrieve (default: 5, max: 10)
    #[serde(default = "default_rag_top_k")]
    pub top_k: Option<usize>,
    /// Optional company filter
    pub company_id: Option<Uuid>,
    /// Optional document filter
    pub document_id: Option<Uuid>,
    /// LLM model to use (default: anthropic/claude-3.5-sonnet)
    /// Available models: anthropic/claude-3.5-sonnet, openai/gpt-4o, openai/gpt-4o-mini
    pub model: Option<String>,
    /// LLM temperature (default: 0.7)
    pub temperature: Option<f32>,
    /// Max tokens for LLM response (default: 1500)
    pub max_tokens: Option<u32>,
}

fn default_rag_top_k() -> Option<usize> {
    Some(5)
}

/// RAG query response
#[derive(Debug, Serialize, ToSchema)]
pub struct RagQueryResponse {
    /// The AI-generated answer
    pub answer: String,
    /// Sources used to generate the answer
    pub sources: Vec<SourceDto>,
    /// Total tokens used in LLM call
    pub total_tokens: Option<u32>,
}

/// Source information DTO
#[derive(Debug, Serialize, ToSchema)]
pub struct SourceDto {
    /// Document ID
    pub document_id: Uuid,
    /// Company ID
    pub company_id: Uuid,
    /// Deal ID if applicable
    pub deal_id: Option<Uuid>,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
    /// The relevant text excerpt
    pub text: String,
    /// Chunk index
    pub chunk_index: usize,
    /// Page number if available
    pub page_number: Option<i32>,
}

// ============================================================================
// API ENDPOINTS
// ============================================================================

/// Vector similarity search endpoint
/// POST /api/v1/documents/search
#[utoipa::path(
    post,
    path = "/api/v1/documents/search",
    request_body = VectorSearchRequest,
    responses(
        (status = 200, description = "Search results returned successfully", body = VectorSearchResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Service unavailable - SurrealDB or embedding service not configured")
    ),
    tag = "Documents - AI Search",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn vector_search(
    State(state): State<ApiState>,
    Json(request): Json<VectorSearchRequest>,
) -> Result<Json<VectorSearchResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        query = %request.query,
        top_k = request.top_k,
        company_id = ?request.company_id,
        document_id = ?request.document_id,
        "Vector search request received"
    );

    // Validate top_k
    let top_k = request.top_k.min(20);

    // Get SurrealDB service
    let surrealdb = state.surrealdb_service.as_ref().ok_or_else(|| {
        error!("SurrealDB service not available");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "service_unavailable".to_string(),
                message: "SurrealDB service not configured".to_string(),
                details: None,
            }),
        )
    })?;

    // Get embedding service (from document_ingestion module)
    let embedding_service = crate::services::EmbeddingService::from_env().map_err(|e| {
        error!("Failed to initialize embedding service: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "service_error".to_string(),
                message: format!("Embedding service initialization failed: {}", e),
                details: None,
            }),
        )
    })?;

    // Generate embedding for query
    let query_embedding = embedding_service.embed(&request.query).await.map_err(|e| {
        error!("Failed to generate query embedding: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "embedding_error".to_string(),
                message: format!("Failed to generate embedding: {}", e),
                details: None,
            }),
        )
    })?;

    // Search based on filters
    let search_results = if let Some(document_id) = request.document_id {
        surrealdb
            .search_by_document(document_id, query_embedding, top_k)
            .await
    } else if let Some(company_id) = request.company_id {
        surrealdb
            .search_by_company(Some(company_id), query_embedding, top_k)
            .await
    } else {
        surrealdb.search(query_embedding, top_k).await
    }
    .map_err(|e| {
        error!("Vector search failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "search_error".to_string(),
                message: format!("Search failed: {}", e),
                details: None,
            }),
        )
    })?;

    let results: Vec<SearchResultDto> = search_results
        .into_iter()
        .map(|r| SearchResultDto {
            document_id: r.document_id,
            company_id: r.company_id,
            deal_id: r.deal_id,
            score: r.score,
            text: r.chunk_text,
            chunk_index: r.chunk_index,
            chunk_position: r.chunk_position,
            page_number: r.page_number,
        })
        .collect();

    let count = results.len();

    info!(
        query = %request.query,
        results_count = count,
        "Vector search completed successfully"
    );

    Ok(Json(VectorSearchResponse { results, count }))
}

/// RAG query endpoint - Ask questions about documents
/// POST /api/v1/documents/ask
#[utoipa::path(
    post,
    path = "/api/v1/documents/ask",
    request_body = RagQueryRequest,
    responses(
        (status = 200, description = "Answer generated successfully", body = RagQueryResponse),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Service unavailable - RAG services not configured")
    ),
    tag = "Documents - AI Search",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn rag_query(
    State(state): State<ApiState>,
    Json(request): Json<RagQueryRequest>,
) -> Result<Json<RagQueryResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        question = %request.question,
        top_k = ?request.top_k,
        company_id = ?request.company_id,
        document_id = ?request.document_id,
        "RAG query request received"
    );

    // Validate and clamp top_k
    let top_k = request.top_k.map(|k| k.min(10));

    // Get services
    let surrealdb = state.surrealdb_service.as_ref().ok_or_else(|| {
        error!("SurrealDB service not available");
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "service_unavailable".to_string(),
                message: "SurrealDB service not configured".to_string(),
                details: None,
            }),
        )
    })?;

    // Initialize services
    let embedding_service =
        std::sync::Arc::new(crate::services::EmbeddingService::from_env().map_err(|e| {
            error!("Failed to initialize embedding service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "service_error".to_string(),
                    message: format!("Embedding service initialization failed: {}", e),
                    details: None,
                }),
            )
        })?);

    let openrouter_service =
        std::sync::Arc::new(crate::services::OpenRouterService::from_env().map_err(|e| {
            error!("Failed to initialize OpenRouter service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "service_error".to_string(),
                    message: format!("OpenRouter service initialization failed: {}", e),
                    details: None,
                }),
            )
        })?);

    let rag_service =
        crate::services::RagService::new(surrealdb.clone(), embedding_service, openrouter_service);

    // Execute RAG query
    let rag_query = crate::services::RagQuery {
        question: request.question.clone(),
        company_id: request.company_id,
        document_id: request.document_id,
        top_k,
        model: request.model,
        temperature: request.temperature,
        max_tokens: request.max_tokens,
    };

    let result = rag_service.query(rag_query).await.map_err(|e| {
        error!("RAG query failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "rag_error".to_string(),
                message: format!("RAG query failed: {}", e),
                details: None,
            }),
        )
    })?;

    let sources: Vec<SourceDto> = result
        .sources
        .into_iter()
        .map(|s| SourceDto {
            document_id: s.document_id,
            company_id: s.company_id,
            deal_id: s.deal_id,
            score: s.score,
            text: s.text,
            chunk_index: s.chunk_index,
            page_number: s.page_number,
        })
        .collect();

    info!(
        question = %request.question,
        sources_count = sources.len(),
        tokens = ?result.total_tokens,
        "RAG query completed successfully"
    );

    Ok(Json(RagQueryResponse {
        answer: result.answer,
        sources,
        total_tokens: result.total_tokens,
    }))
}

/// Streaming RAG query endpoint - Ask questions about documents with streaming response
/// POST /api/v1/documents/ask/stream
#[utoipa::path(
    post,
    path = "/api/v1/documents/ask/stream",
    request_body = RagQueryRequest,
    responses(
        (status = 200, description = "Streaming answer generation in progress", content_type = "text/event-stream"),
        (status = 400, description = "Invalid request parameters"),
        (status = 500, description = "Internal server error"),
        (status = 503, description = "Service unavailable - RAG services not configured")
    ),
    tag = "Documents - AI Search",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn rag_query_stream(
    State(state): State<ApiState>,
    Json(request): Json<RagQueryRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        question = %request.question,
        top_k = ?request.top_k,
        model = ?request.model,
        company_id = ?request.company_id,
        document_id = ?request.document_id,
        "RAG streaming query request received"
    );

    // Validate and clamp top_k
    let top_k = request.top_k.map(|k| k.min(10));

    // Get services (clone the Arc to extend lifetime)
    let surrealdb = state
        .surrealdb_service
        .as_ref()
        .ok_or_else(|| {
            error!("SurrealDB service not available");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "service_unavailable".to_string(),
                    message: "SurrealDB service not configured".to_string(),
                    details: None,
                }),
            )
        })?
        .clone();

    // Initialize services
    let embedding_service =
        std::sync::Arc::new(crate::services::EmbeddingService::from_env().map_err(|e| {
            error!("Failed to initialize embedding service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "service_error".to_string(),
                    message: format!("Embedding service initialization failed: {}", e),
                    details: None,
                }),
            )
        })?);

    let openrouter_service =
        std::sync::Arc::new(crate::services::OpenRouterService::from_env().map_err(|e| {
            error!("Failed to initialize OpenRouter service: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "service_error".to_string(),
                    message: format!("OpenRouter service initialization failed: {}", e),
                    details: None,
                }),
            )
        })?);

    // Store request data for async processing
    let question = request.question.clone();
    let model = request.model.clone();
    let company_id = request.company_id;
    let document_id = request.document_id;
    let temperature = request.temperature;
    let max_tokens = request.max_tokens;

    // Create the stream
    let stream = async_stream::stream! {
        // 1. Generate embedding for query
        let query_embedding = match embedding_service.embed(&question).await {
            Ok(emb) => emb,
            Err(e) => {
                error!("Failed to generate query embedding: {}", e);
                yield Ok(Event::default().data(format!("Error: Failed to generate embedding: {}", e)));
                return;
            }
        };

        // 2. Search for relevant chunks
        let search_results = if let Some(doc_id) = document_id {
            surrealdb.search_by_document(doc_id, query_embedding, top_k.unwrap_or(5)).await
        } else if let Some(comp_id) = company_id {
            surrealdb.search_by_company(Some(comp_id), query_embedding, top_k.unwrap_or(5)).await
        } else {
            surrealdb.search(query_embedding, top_k.unwrap_or(5)).await
        };

        let search_results = match search_results {
            Ok(results) => results,
            Err(e) => {
                error!("Vector search failed: {}", e);
                yield Ok(Event::default().data(format!("Error: Vector search failed: {}", e)));
                return;
            }
        };

        if search_results.is_empty() {
            yield Ok(Event::default().data("I couldn't find any relevant information in the documents to answer your question."));
            yield Ok(Event::default().event("done").data(""));
            return;
        }

        // 3. Build context
        let mut context = String::new();
        for (i, result) in search_results.iter().enumerate() {
            context.push_str(&format!(
                "\n--- Document Chunk {} (Score: {:.3}) ---\n",
                i + 1,
                result.score
            ));
            if let Some(page) = result.page_number {
                context.push_str(&format!("Page: {}\n", page));
            }
            context.push_str(&result.chunk_text);
            context.push_str("\n");
        }

        // 4. Build messages for LLM
        let messages = vec![
            crate::services::openrouter_service::ChatMessage {
                role: "system".to_string(),
                content: r#"You are an AI assistant for Arda Credit, a privacy-preserving credit platform.

Your role is to help users understand and analyze financial documents, credit agreements, and deal information.

Guidelines:
- Base your answers strictly on the provided context
- Be precise and cite specific information when possible
- If information is unclear or missing, state that explicitly
- Use professional financial terminology
- Maintain confidentiality and privacy of sensitive information
- For numerical data, be exact and include relevant details (amounts, dates, percentages)
- If asked about compliance or legal matters, recommend consulting qualified professionals"#.to_string(),
            },
            crate::services::openrouter_service::ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Context from relevant documents:\n{}\n\nQuestion: {}\n\nPlease provide a comprehensive answer based on the context above. If the context doesn't contain enough information to answer the question, say so.",
                    context, question
                ),
            },
        ];

        // 5. Stream LLM response
        let llm_stream = if let Some(model_name) = model {
            openrouter_service.stream_completion_with_model(
                &model_name,
                messages,
                temperature.or(Some(0.7)),
                max_tokens.or(Some(1500)),
            ).await
        } else {
            openrouter_service.stream_completion(
                messages,
                temperature.or(Some(0.7)),
                max_tokens.or(Some(1500)),
            ).await
        };

        let mut llm_stream = match llm_stream {
            Ok(stream) => stream,
            Err(e) => {
                error!("Failed to create LLM stream: {}", e);
                yield Ok(Event::default().data(format!("Error: Failed to generate response: {}", e)));
                return;
            }
        };

        // Stream chunks to client
        while let Some(chunk_result) = llm_stream.next().await {
            match chunk_result {
                Ok(event) => {
                    if event.done {
                        // Send sources as final event
                        let sources: Vec<SourceDto> = search_results
                            .iter()
                            .map(|r| SourceDto {
                                document_id: r.document_id,
                                company_id: r.company_id,
                                deal_id: r.deal_id,
                                score: r.score,
                                text: r.chunk_text.clone(),
                                chunk_index: r.chunk_index,
                                page_number: r.page_number,
                            })
                            .collect();

                        if let Ok(sources_json) = serde_json::to_string(&sources) {
                            yield Ok(Event::default().event("sources").data(sources_json));
                        }

                        yield Ok(Event::default().event("done").data(""));
                        break;
                    } else if !event.content.is_empty() {
                        yield Ok(Event::default().data(event.content));
                    }
                }
                Err(e) => {
                    error!("Stream error: {}", e);
                    yield Ok(Event::default().data(format!("Error: {}", e)));
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream))
}
