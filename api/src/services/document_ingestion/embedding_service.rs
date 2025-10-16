use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info};

/// OpenAI embedding models
#[derive(Debug, Clone, Copy)]
pub enum EmbeddingModel {
    /// text-embedding-3-small (1536 dimensions, $0.02/1M tokens)
    TextEmbedding3Small,
    /// text-embedding-3-large (3072 dimensions, $0.13/1M tokens)
    TextEmbedding3Large,
    /// text-embedding-ada-002 (1536 dimensions, legacy)
    Ada002,
}

impl EmbeddingModel {
    pub fn as_str(&self) -> &str {
        match self {
            EmbeddingModel::TextEmbedding3Small => "text-embedding-3-small",
            EmbeddingModel::TextEmbedding3Large => "text-embedding-3-large",
            EmbeddingModel::Ada002 => "text-embedding-ada-002",
        }
    }

    pub fn dimensions(&self) -> usize {
        match self {
            EmbeddingModel::TextEmbedding3Small => 1536,
            EmbeddingModel::TextEmbedding3Large => 3072,
            EmbeddingModel::Ada002 => 1536,
        }
    }
}

/// OpenAI API request for embeddings
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    input: Vec<String>,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

/// OpenAI API response for embeddings
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: usize,
    total_tokens: usize,
}

/// Embedding Service for converting text to vectors
///
/// Uses OpenAI's embedding API to generate vector representations
/// of text chunks for semantic search.
pub struct EmbeddingService {
    client: Client,
    api_key: String,
    model: EmbeddingModel,
    base_url: String,
}

impl EmbeddingService {
    /// Create a new embedding service from environment variables
    ///
    /// Required env vars:
    /// - OPENAI_API_KEY
    ///
    /// Optional env vars:
    /// - EMBEDDING_MODEL (default: text-embedding-3-small)
    /// - OPENAI_API_BASE_URL (default: https://api.openai.com/v1)
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;

        let model_str = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-3-small".to_string());

        let model = match model_str.as_str() {
            "text-embedding-3-small" => EmbeddingModel::TextEmbedding3Small,
            "text-embedding-3-large" => EmbeddingModel::TextEmbedding3Large,
            "text-embedding-ada-002" => EmbeddingModel::Ada002,
            _ => {
                anyhow::bail!("Unknown embedding model: {}", model_str);
            }
        };

        let base_url = std::env::var("OPENAI_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        info!(
            "Initializing EmbeddingService with model: {}",
            model.as_str()
        );

        Self::new(api_key, model, base_url)
    }

    /// Create a new embedding service with explicit configuration
    pub fn new(api_key: String, model: EmbeddingModel, base_url: String) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        Ok(Self {
            client,
            api_key,
            model,
            base_url,
        })
    }

    /// Get the dimensions for the current embedding model
    pub fn dimensions(&self) -> usize {
        self.model.dimensions()
    }

    /// Generate embedding for a single text
    ///
    /// # Arguments
    /// * `text` - The text to embed
    ///
    /// # Returns
    /// * Vector of floats representing the embedding
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(vec![text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .context("No embedding returned")
    }

    /// Generate embeddings for multiple texts in batch
    ///
    /// More efficient than calling embed() multiple times.
    /// OpenAI allows up to 2048 texts per batch.
    ///
    /// # Arguments
    /// * `texts` - Vector of texts to embed
    ///
    /// # Returns
    /// * Vector of embeddings in the same order as input texts
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        debug!("Generating embeddings for {} texts", texts.len());

        // OpenAI limits batch size to 2048
        if texts.len() > 2048 {
            anyhow::bail!("Batch size exceeds OpenAI limit of 2048");
        }

        // Build request
        let request = EmbeddingRequest {
            input: texts,
            model: self.model.as_str().to_string(),
            dimensions: None, // Use model default
        };

        // Call OpenAI API
        let url = format!("{}/embeddings", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send embedding request")?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenAI API error ({}): {}", status, error_text);
        }

        // Parse response
        let embedding_response: EmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        debug!(
            "Generated {} embeddings (used {} tokens)",
            embedding_response.data.len(),
            embedding_response.usage.total_tokens
        );

        // Sort by index and extract embeddings
        let mut data = embedding_response.data;
        data.sort_by_key(|d| d.index);
        let embeddings: Vec<Vec<f32>> = data.into_iter().map(|d| d.embedding).collect();

        Ok(embeddings)
    }

    /// Generate embeddings in chunks to handle large batches
    ///
    /// Automatically splits large batches into multiple API calls
    /// to stay within OpenAI's batch size limit.
    ///
    /// # Arguments
    /// * `texts` - Vector of texts to embed
    /// * `chunk_size` - Size of each batch (default: 100)
    ///
    /// # Returns
    /// * Vector of embeddings in the same order as input texts
    pub async fn embed_batch_chunked(
        &self,
        texts: Vec<String>,
        chunk_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        info!(
            "Generating embeddings for {} texts in chunks of {}",
            texts.len(),
            chunk_size
        );

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(chunk_size) {
            let chunk_embeddings = self.embed_batch(chunk.to_vec()).await?;
            all_embeddings.extend(chunk_embeddings);

            // Small delay to avoid rate limits
            if texts.len() > chunk_size {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        Ok(all_embeddings)
    }

    /// Health check - verify API key is valid
    pub async fn health_check(&self) -> Result<bool> {
        // Try embedding a simple test string
        match self.embed("test").await {
            Ok(_) => Ok(true),
            Err(e) => {
                debug!("Embedding health check failed: {}", e);
                Ok(false)
            }
        }
    }
}

/// Calculate cosine similarity between two embeddings
///
/// Returns a value between -1 and 1, where 1 means identical.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_model_dimensions() {
        assert_eq!(EmbeddingModel::TextEmbedding3Small.dimensions(), 1536);
        assert_eq!(EmbeddingModel::TextEmbedding3Large.dimensions(), 3072);
        assert_eq!(EmbeddingModel::Ada002.dimensions(), 1536);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - (-1.0)).abs() < 0.001);

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_embedding_generation() {
        let service = EmbeddingService::from_env().expect("Failed to create service");
        let embedding = service
            .embed("This is a test")
            .await
            .expect("Failed to generate embedding");

        assert_eq!(embedding.len(), service.dimensions());
    }

    #[tokio::test]
    #[ignore] // Requires API key
    async fn test_batch_embedding() {
        let service = EmbeddingService::from_env().expect("Failed to create service");
        let texts = vec![
            "First text".to_string(),
            "Second text".to_string(),
            "Third text".to_string(),
        ];

        let embeddings = service
            .embed_batch(texts.clone())
            .await
            .expect("Failed to generate embeddings");

        assert_eq!(embeddings.len(), texts.len());
        for embedding in &embeddings {
            assert_eq!(embedding.len(), service.dimensions());
        }
    }
}
