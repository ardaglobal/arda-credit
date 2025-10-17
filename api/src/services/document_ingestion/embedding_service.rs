use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Embedding provider
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingProvider {
    /// OpenAI API
    OpenAI,
    /// Modal custom endpoint
    Modal,
}

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
    total_tokens: usize,
}

/// Embedding Service for converting text to vectors
///
/// Supports multiple providers:
/// - OpenAI API (default)
/// - Modal custom endpoint (for hosted models)
pub struct EmbeddingService {
    client: Client,
    provider: EmbeddingProvider,
    api_key: Option<String>,
    model: EmbeddingModel,
    base_url: String,
}

impl EmbeddingService {
    /// Create a new embedding service from environment variables
    ///
    /// Required env vars (depending on provider):
    /// - For OpenAI: OPENAI_API_KEY
    /// - For Modal: MODAL_EMBEDDING_URL (defaults to https://ardaglobal--embed.modal.run/)
    ///
    /// Optional env vars:
    /// - EMBEDDING_PROVIDER (openai|modal, default: modal)
    /// - EMBEDDING_MODEL (default: text-embedding-3-small)
    /// - OPENAI_API_BASE_URL (default: https://api.openai.com/v1)
    pub fn from_env() -> Result<Self> {
        let provider_str = std::env::var("EMBEDDING_PROVIDER")
            .unwrap_or_else(|_| "modal".to_string())
            .to_lowercase();

        let provider = match provider_str.as_str() {
            "openai" => EmbeddingProvider::OpenAI,
            "modal" => EmbeddingProvider::Modal,
            _ => {
                warn!(
                    "Unknown embedding provider '{}', defaulting to Modal",
                    provider_str
                );
                EmbeddingProvider::Modal
            }
        };

        let model_str = std::env::var("EMBEDDING_MODEL")
            .unwrap_or_else(|_| "text-embedding-3-small".to_string());

        let model = match model_str.as_str() {
            "text-embedding-3-small" => EmbeddingModel::TextEmbedding3Small,
            "text-embedding-3-large" => EmbeddingModel::TextEmbedding3Large,
            "text-embedding-ada-002" => EmbeddingModel::Ada002,
            _ => {
                warn!(
                    "Unknown embedding model '{}', defaulting to text-embedding-3-small",
                    model_str
                );
                EmbeddingModel::TextEmbedding3Small
            }
        };

        let (api_key, base_url) = match provider {
            EmbeddingProvider::OpenAI => {
                let key = std::env::var("OPENAI_API_KEY")
                    .context("OPENAI_API_KEY required when using OpenAI provider")?;
                let url = std::env::var("OPENAI_API_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
                (Some(key), url)
            }
            EmbeddingProvider::Modal => {
                let url = std::env::var("MODAL_EMBEDDING_URL")
                    .unwrap_or_else(|_| "https://ardaglobal--embed.modal.run/".to_string());
                (None, url) // Modal endpoint may not require API key
            }
        };

        info!(
            "Initializing EmbeddingService with provider: {:?}, model: {}",
            provider,
            model.as_str()
        );

        Self::new(provider, api_key, model, base_url)
    }

    /// Create a new embedding service with explicit configuration
    pub fn new(
        provider: EmbeddingProvider,
        api_key: Option<String>,
        model: EmbeddingModel,
        base_url: String,
    ) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        Ok(Self {
            client,
            provider,
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

        // Check batch size limit
        if texts.len() > 2048 {
            anyhow::bail!("Batch size exceeds limit of 2048");
        }

        match self.provider {
            EmbeddingProvider::OpenAI => self.embed_batch_openai(texts).await,
            EmbeddingProvider::Modal => self.embed_batch_modal(texts).await,
        }
    }

    /// Generate embeddings using OpenAI API
    async fn embed_batch_openai(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let api_key = self
            .api_key
            .as_ref()
            .context("API key required for OpenAI provider")?;

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
            .header("Authorization", format!("Bearer {}", api_key))
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

    /// Generate embeddings using Modal endpoint
    async fn embed_batch_modal(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        // Modal endpoint expects a simpler format
        #[derive(Serialize)]
        struct ModalRequest {
            texts: Vec<String>,
        }

        #[derive(Deserialize)]
        struct ModalResponse {
            embeddings: Vec<Vec<f32>>,
        }

        let request = ModalRequest { texts };

        // Call Modal endpoint
        let url = if self.base_url.ends_with('/') {
            format!("{}embed", self.base_url)
        } else {
            format!("{}/embed", self.base_url)
        };

        let mut req = self.client.post(&url).json(&request);

        // Add auth header if API key is provided
        if let Some(api_key) = &self.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req
            .send()
            .await
            .context("Failed to send embedding request to Modal")?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Modal API error ({}): {}", status, error_text);
        }

        // Parse response
        let modal_response: ModalResponse = response
            .json()
            .await
            .context("Failed to parse Modal embedding response")?;

        debug!(
            "Generated {} embeddings from Modal",
            modal_response.embeddings.len()
        );

        Ok(modal_response.embeddings)
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
