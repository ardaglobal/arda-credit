///! RAG (Retrieval-Augmented Generation) Service
///!
///! Combines vector similarity search with LLM inference to provide
///! intelligent question-answering over document collections.
use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use super::document_ingestion::{EmbeddingService, SearchResult, SurrealDBService};
use super::openrouter_service::{ChatMessage, OpenRouterService};

/// RAG service for document-based Q&A
pub struct RagService {
    surrealdb: Arc<SurrealDBService>,
    embedding: Arc<EmbeddingService>,
    llm: Arc<OpenRouterService>,
}

/// RAG query request
#[derive(Debug, Clone)]
pub struct RagQuery {
    /// The user's question
    pub question: String,
    /// Optional company filter
    pub company_id: Option<Uuid>,
    /// Optional document filter
    pub document_id: Option<Uuid>,
    /// Number of relevant chunks to retrieve (default: 5)
    pub top_k: Option<usize>,
    /// LLM temperature (default: 0.7)
    pub temperature: Option<f32>,
    /// Max tokens for LLM response (default: 1500)
    pub max_tokens: Option<u32>,
}

/// RAG query result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RagResult {
    /// The LLM's answer to the question
    pub answer: String,
    /// Sources used to generate the answer
    pub sources: Vec<Source>,
    /// Total tokens used in LLM call
    pub total_tokens: Option<u32>,
}

/// Source information for a RAG result
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Source {
    /// Document ID
    pub document_id: Uuid,
    /// Company ID
    pub company_id: Uuid,
    /// Deal ID if applicable
    pub deal_id: Option<Uuid>,
    /// Relevance score (0.0 to 1.0)
    pub score: f32,
    /// The relevant text chunk
    pub text: String,
    /// Chunk index in document
    pub chunk_index: usize,
    /// Page number if applicable
    pub page_number: Option<i32>,
}

impl RagService {
    /// Create a new RAG service
    pub fn new(
        surrealdb: Arc<SurrealDBService>,
        embedding: Arc<EmbeddingService>,
        llm: Arc<OpenRouterService>,
    ) -> Self {
        info!("Initializing RAG service");
        Self {
            surrealdb,
            embedding,
            llm,
        }
    }

    /// Query documents with RAG
    pub async fn query(&self, query: RagQuery) -> Result<RagResult> {
        info!(
            "Processing RAG query: '{}' (company: {:?}, document: {:?})",
            query.question, query.company_id, query.document_id
        );

        // 1. Generate embedding for the question
        debug!("Generating embedding for question");
        let query_embedding = self
            .embedding
            .embed(&query.question)
            .await
            .context("Failed to generate query embedding")?;

        // 2. Search for relevant chunks
        let top_k = query.top_k.unwrap_or(5);
        debug!("Searching for top {} relevant chunks", top_k);

        let search_results = if let Some(document_id) = query.document_id {
            // Search within specific document
            self.surrealdb
                .search_by_document(document_id, query_embedding, top_k)
                .await?
        } else if let Some(company_id) = query.company_id {
            // Search within company's documents
            self.surrealdb
                .search_by_company(Some(company_id), query_embedding, top_k)
                .await?
        } else {
            // Search across all documents
            self.surrealdb.search(query_embedding, top_k).await?
        };

        if search_results.is_empty() {
            return Ok(RagResult {
                answer: "I couldn't find any relevant information in the documents to answer your question.".to_string(),
                sources: Vec::new(),
                total_tokens: None,
            });
        }

        debug!("Found {} relevant chunks", search_results.len());

        // 3. Build context from search results
        let context = self.build_context(&search_results);

        // 4. Generate LLM response
        debug!("Generating LLM response");
        let completion = self
            .llm
            .complete(
                self.build_messages(&query.question, &context),
                query.temperature.or(Some(0.7)),
                query.max_tokens.or(Some(1500)),
            )
            .await
            .context("Failed to generate LLM response")?;

        // 5. Build result with sources
        let sources = search_results
            .into_iter()
            .map(|result| Source {
                document_id: result.document_id,
                company_id: result.company_id,
                deal_id: result.deal_id,
                score: result.score,
                text: result.chunk_text,
                chunk_index: result.chunk_index,
                page_number: result.page_number,
            })
            .collect();

        Ok(RagResult {
            answer: completion.content,
            sources,
            total_tokens: completion.total_tokens,
        })
    }

    /// Build context string from search results
    fn build_context(&self, results: &[SearchResult]) -> String {
        let mut context = String::new();

        for (i, result) in results.iter().enumerate() {
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

        context
    }

    /// Build chat messages for LLM
    fn build_messages(&self, question: &str, context: &str) -> Vec<ChatMessage> {
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: self.build_system_prompt(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!(
                    "Context from relevant documents:\n{}\n\nQuestion: {}\n\nPlease provide a comprehensive answer based on the context above. If the context doesn't contain enough information to answer the question, say so.",
                    context, question
                ),
            },
        ]
    }

    /// Build system prompt for LLM
    fn build_system_prompt(&self) -> String {
        r#"You are an AI assistant for Arda Credit, a privacy-preserving credit platform.

Your role is to help users understand and analyze financial documents, credit agreements, and deal information.

Guidelines:
- Base your answers strictly on the provided context
- Be precise and cite specific information when possible
- If information is unclear or missing, state that explicitly
- Use professional financial terminology
- Maintain confidentiality and privacy of sensitive information
- For numerical data, be exact and include relevant details (amounts, dates, percentages)
- If asked about compliance or legal matters, recommend consulting qualified professionals"#.to_string()
    }

    /// Simple question-answering without filtering
    pub async fn ask(&self, question: &str, top_k: Option<usize>) -> Result<RagResult> {
        self.query(RagQuery {
            question: question.to_string(),
            company_id: None,
            document_id: None,
            top_k,
            temperature: None,
            max_tokens: None,
        })
        .await
    }

    /// Ask a question about a specific document
    pub async fn ask_document(
        &self,
        document_id: Uuid,
        question: &str,
        top_k: Option<usize>,
    ) -> Result<RagResult> {
        self.query(RagQuery {
            question: question.to_string(),
            company_id: None,
            document_id: Some(document_id),
            top_k,
            temperature: None,
            max_tokens: None,
        })
        .await
    }

    /// Ask a question scoped to a company's documents
    pub async fn ask_company(
        &self,
        company_id: Uuid,
        question: &str,
        top_k: Option<usize>,
    ) -> Result<RagResult> {
        self.query(RagQuery {
            question: question.to_string(),
            company_id: Some(company_id),
            document_id: None,
            top_k,
            temperature: None,
            max_tokens: None,
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context() {
        // This would require mock search results
        // Skipping for now as it needs full setup
    }

    #[test]
    fn test_system_prompt_contains_guidelines() {
        let rag = RagService {
            surrealdb: Arc::new(unsafe { std::mem::zeroed() }),
            embedding: Arc::new(unsafe { std::mem::zeroed() }),
            llm: Arc::new(unsafe { std::mem::zeroed() }),
        };

        let prompt = rag.build_system_prompt();
        assert!(prompt.contains("Arda Credit"));
        assert!(prompt.contains("Guidelines"));
    }
}
