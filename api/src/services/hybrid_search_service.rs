///! Hybrid Search Service
///!
///! Combines vector similarity search with keyword search (BM25) for optimal results.
///! Uses reciprocal rank fusion (RRF) to merge results from both methods.
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use super::document_ingestion::{EmbeddingService, SearchResult, SurrealDBService};

/// Hybrid search service combining vector and keyword search
pub struct HybridSearchService {
    surrealdb: Arc<SurrealDBService>,
    embedding: Arc<EmbeddingService>,
}

/// Hybrid search configuration
#[derive(Debug, Clone)]
pub struct HybridSearchConfig {
    /// Weight for vector search results (0.0 to 1.0)
    pub vector_weight: f32,
    /// Weight for keyword search results (0.0 to 1.0)
    pub keyword_weight: f32,
    /// Minimum relevance score to include in results (0.0 to 1.0)
    pub min_score: f32,
    /// Number of results from each method before fusion
    pub k_vector: usize,
    /// Number of results from keyword search
    pub k_keyword: usize,
}

impl Default for HybridSearchConfig {
    fn default() -> Self {
        Self {
            vector_weight: 0.7,  // Favor vector search slightly
            keyword_weight: 0.3, // But still use keyword signals
            min_score: 0.3,      // Filter out low-quality matches
            k_vector: 15,        // Get more candidates for fusion
            k_keyword: 15,
        }
    }
}

/// Hybrid search query
#[derive(Debug, Clone)]
pub struct HybridSearchQuery {
    /// The search query text
    pub query: String,
    /// Optional company filter
    pub company_id: Option<Uuid>,
    /// Optional document filter
    pub document_id: Option<Uuid>,
    /// Optional user filter
    pub user_id: Option<Uuid>,
    /// Number of final results to return
    pub top_k: usize,
    /// Search configuration
    pub config: Option<HybridSearchConfig>,
}

impl HybridSearchService {
    /// Create a new hybrid search service
    pub fn new(surrealdb: Arc<SurrealDBService>, embedding: Arc<EmbeddingService>) -> Self {
        info!("Initializing Hybrid Search Service");
        Self {
            surrealdb,
            embedding,
        }
    }

    /// Perform hybrid search combining vector and keyword methods
    pub async fn search(&self, query: HybridSearchQuery) -> Result<Vec<SearchResult>> {
        let config = query.config.clone().unwrap_or_default();

        info!(
            "Hybrid search: '{}' (vector_weight: {}, keyword_weight: {})",
            query.query, config.vector_weight, config.keyword_weight
        );

        // 1. Vector search
        let vector_results = self.vector_search(&query, config.k_vector).await?;
        debug!("Vector search returned {} results", vector_results.len());

        // 2. Keyword search
        let keyword_results = self.keyword_search(&query, config.k_keyword).await?;
        debug!("Keyword search returned {} results", keyword_results.len());

        // 3. Fuse results using Reciprocal Rank Fusion (RRF)
        let fused_results = self.fuse_results(
            vector_results,
            keyword_results,
            config.vector_weight,
            config.keyword_weight,
            config.min_score,
        );

        // 4. Take top K
        let final_results: Vec<SearchResult> =
            fused_results.into_iter().take(query.top_k).collect();

        info!(
            "Hybrid search returned {} final results",
            final_results.len()
        );

        Ok(final_results)
    }

    /// Perform vector similarity search
    async fn vector_search(
        &self,
        query: &HybridSearchQuery,
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        // Generate embedding for query
        let query_embedding = self
            .embedding
            .embed(&query.query)
            .await
            .context("Failed to generate query embedding")?;

        // Search based on filters
        let results = if let Some(document_id) = query.document_id {
            self.surrealdb
                .search_by_document(document_id, query_embedding, k)
                .await?
        } else if let Some(company_id) = query.company_id {
            self.surrealdb
                .search_by_company(Some(company_id), query_embedding, k)
                .await?
        } else {
            self.surrealdb.search(query_embedding, k).await?
        };

        Ok(results)
    }

    /// Perform keyword search using SurrealDB full-text search
    async fn keyword_search(
        &self,
        query: &HybridSearchQuery,
        k: usize,
    ) -> Result<Vec<SearchResult>> {
        debug!("Performing keyword search for: '{}'", query.query);

        // Build keyword search query
        let search_query =
            self.build_keyword_query(&query.query, query.company_id, query.document_id, k);

        // Execute keyword search
        let results = self.execute_keyword_query(search_query).await?;

        Ok(results)
    }

    /// Build SurrealDB query for keyword search
    fn build_keyword_query(
        &self,
        query_text: &str,
        company_id: Option<Uuid>,
        document_id: Option<Uuid>,
        limit: usize,
    ) -> String {
        // Extract keywords from query (simple tokenization)
        let keywords = self.extract_keywords(query_text);
        let keyword_pattern = keywords.join("|");

        // Build WHERE clause
        let mut where_clauses = vec![format!("chunk_text ~* '{}'", keyword_pattern)];

        if let Some(cid) = company_id {
            where_clauses.push(format!("company_id = '{}'", cid));
        }

        if let Some(did) = document_id {
            where_clauses.push(format!("document_id = '{}'", did));
        }

        let where_clause = where_clauses.join(" AND ");

        // Build full query
        format!(
            "SELECT embedding_id, document_id, company_id, deal_id, chunk_text, \
             chunk_index, chunk_position, page_number, \
             1.0 as score \
             FROM document_embeddings \
             WHERE {} \
             LIMIT {}",
            where_clause, limit
        )
    }

    /// Execute keyword search query
    async fn execute_keyword_query(&self, query: String) -> Result<Vec<SearchResult>> {
        // For now, return empty results - full-text search requires SurrealDB configuration
        // In production, this would execute the actual keyword search
        debug!("Keyword search query: {}", query);

        // This is a simplified implementation - in production you'd want:
        // 1. Full-text indexing in SurrealDB
        // 2. BM25 scoring
        // 3. Proper tokenization and stemming

        Ok(Vec::new())
    }

    /// Extract keywords from query text
    fn extract_keywords(&self, text: &str) -> Vec<String> {
        Self::extract_keywords_static(text)
    }

    /// Static helper for keyword extraction (for testing)
    fn extract_keywords_static(text: &str) -> Vec<String> {
        // Simple keyword extraction - split on whitespace and filter stopwords
        let stopwords = vec![
            "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with",
            "by", "from", "is", "are", "was", "were", "be", "been", "what", "who", "where", "when",
            "why", "how", "this", "that", "these", "those",
        ];

        text.split_whitespace()
            .map(|word| {
                // Strip punctuation from both ends and convert to lowercase
                word.trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase()
            })
            .filter(|word| !word.is_empty() && !stopwords.contains(&word.as_str()))
            .collect()
    }

    /// Fuse results from vector and keyword search using Reciprocal Rank Fusion
    fn fuse_results(
        &self,
        vector_results: Vec<SearchResult>,
        keyword_results: Vec<SearchResult>,
        vector_weight: f32,
        keyword_weight: f32,
        min_score: f32,
    ) -> Vec<SearchResult> {
        // Create a map of embedding_id to result
        let mut result_map: HashMap<Uuid, SearchResult> = HashMap::new();
        let mut score_map: HashMap<Uuid, f32> = HashMap::new();

        // RRF constant (typically 60)
        let rrf_k = 60.0;

        // Process vector results
        for (rank, result) in vector_results.iter().enumerate() {
            let rrf_score = vector_weight / (rrf_k + (rank + 1) as f32);
            score_map.insert(result.embedding_id, rrf_score);
            result_map.insert(result.embedding_id, result.clone());
        }

        // Process keyword results
        for (rank, result) in keyword_results.iter().enumerate() {
            let rrf_score = keyword_weight / (rrf_k + (rank + 1) as f32);
            let embedding_id = result.embedding_id;

            // Add or update score
            score_map
                .entry(embedding_id)
                .and_modify(|score| *score += rrf_score)
                .or_insert(rrf_score);

            // Add to result map if not already present
            result_map
                .entry(embedding_id)
                .or_insert_with(|| result.clone());
        }

        // Convert to vector and sort by fused score
        let mut fused: Vec<(Uuid, f32, SearchResult)> = score_map
            .into_iter()
            .filter_map(|(id, score)| {
                result_map
                    .get(&id)
                    .map(|result| (id, score, result.clone()))
            })
            .filter(|(_, score, _)| *score >= min_score)
            .collect();

        fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Update scores in results
        let final_results: Vec<SearchResult> = fused
            .into_iter()
            .map(|(_, score, mut result)| {
                result.score = score;
                result
            })
            .collect();

        debug!("Fused {} unique results", final_results.len());

        final_results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_keywords() {
        let keywords = HybridSearchService::extract_keywords_static(
            "What is the interest rate for this loan?",
        );
        assert!(keywords.contains(&"interest".to_string()));
        assert!(keywords.contains(&"rate".to_string()));
        assert!(keywords.contains(&"loan".to_string()));
        assert!(!keywords.contains(&"what".to_string())); // stopword
        assert!(!keywords.contains(&"is".to_string())); // stopword
        assert!(!keywords.contains(&"the".to_string())); // stopword
    }

    #[test]
    fn test_default_config() {
        let config = HybridSearchConfig::default();
        assert_eq!(config.vector_weight + config.keyword_weight, 1.0);
        assert!(config.min_score > 0.0);
    }
}
