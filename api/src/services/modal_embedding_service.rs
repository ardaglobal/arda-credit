///! Modal Embedding Service Integration
///!
///! Provides integration with Modal.com for serverless embedding generation.
///! Supports custom embedding models deployed on Modal.
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Modal embedding service
pub struct ModalEmbeddingService {
    client: Client,
    api_url: String,
    api_key: Option<String>,
    model_name: String,
    batch_size: usize,
}

/// Request to Modal embedding endpoint
#[derive(Debug, Serialize)]
struct ModalEmbeddingRequest {
    texts: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

/// Response from Modal embedding endpoint
#[derive(Debug, Deserialize)]
struct ModalEmbeddingResponse {
    embeddings: Vec<Vec<f32>>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    dimensions: Option<usize>,
}

impl ModalEmbeddingService {
    /// Create a new Modal embedding service from environment variables
    ///
    /// Required env vars:
    /// - MODAL_EMBEDDING_URL: URL to your Modal embedding endpoint
    ///
    /// Optional env vars:
    /// - MODAL_API_KEY: API key for authentication
    /// - MODAL_EMBEDDING_MODEL: Model name to use
    /// - MODAL_EMBEDDING_BATCH_SIZE: Batch size for processing (default: 50)
    pub fn from_env() -> Result<Self> {
        let api_url = std::env::var("MODAL_EMBEDDING_URL")
            .context("MODAL_EMBEDDING_URL environment variable is required")?;

        let api_key = std::env::var("MODAL_API_KEY").ok();

        let model_name =
            std::env::var("MODAL_EMBEDDING_MODEL").unwrap_or_else(|_| "default".to_string());

        let batch_size = std::env::var("MODAL_EMBEDDING_BATCH_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(50);

        info!(
            "Initializing Modal embedding service (url: {}, model: {}, batch_size: {})",
            api_url, model_name, batch_size
        );

        Ok(Self {
            client: Client::new(),
            api_url,
            api_key,
            model_name,
            batch_size,
        })
    }

    /// Generate embedding for a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.embed_batch(vec![text.to_string()]).await?;
        embeddings
            .into_iter()
            .next()
            .context("No embedding returned from Modal")
    }

    /// Generate embeddings for a batch of texts
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        debug!("Generating embeddings for {} texts via Modal", texts.len());

        let request = ModalEmbeddingRequest {
            texts,
            model: Some(self.model_name.clone()),
        };

        let mut request_builder = self
            .client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .json(&request);

        // Add API key if configured
        if let Some(api_key) = &self.api_key {
            request_builder =
                request_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request_builder
            .send()
            .await
            .context("Failed to send request to Modal")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!(
                "Modal API error ({}): {}",
                status,
                error_text
            ));
        }

        let modal_response: ModalEmbeddingResponse = response
            .json()
            .await
            .context("Failed to parse Modal response")?;

        debug!(
            "Received {} embeddings from Modal (dimensions: {:?})",
            modal_response.embeddings.len(),
            modal_response.dimensions
        );

        Ok(modal_response.embeddings)
    }

    /// Generate embeddings for a large batch with automatic chunking
    pub async fn embed_batch_chunked(
        &self,
        texts: Vec<String>,
        chunk_size: Option<usize>,
    ) -> Result<Vec<Vec<f32>>> {
        let chunk_size = chunk_size.unwrap_or(self.batch_size);

        info!(
            "Generating embeddings for {} texts in chunks of {}",
            texts.len(),
            chunk_size
        );

        let mut all_embeddings = Vec::new();

        for chunk in texts.chunks(chunk_size) {
            let chunk_embeddings = self.embed_batch(chunk.to_vec()).await?;
            all_embeddings.extend(chunk_embeddings);
        }

        Ok(all_embeddings)
    }

    /// Get the model name being used
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Get the configured batch size
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }
}

/// Unified embedding service that can use either Modal or direct API
pub enum UnifiedEmbeddingService {
    Modal(ModalEmbeddingService),
    Direct(crate::services::EmbeddingService),
}

impl UnifiedEmbeddingService {
    /// Create from environment - tries Modal first, falls back to direct
    pub async fn from_env() -> Result<Self> {
        // Try Modal first
        if std::env::var("MODAL_EMBEDDING_URL").is_ok() {
            info!("Using Modal embedding service");
            Ok(Self::Modal(ModalEmbeddingService::from_env()?))
        } else {
            info!("Using direct embedding service");
            Ok(Self::Direct(crate::services::EmbeddingService::from_env()?))
        }
    }

    /// Generate embedding for a single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match self {
            Self::Modal(service) => service.embed(text).await,
            Self::Direct(service) => service.embed(text).await,
        }
    }

    /// Generate embeddings for a batch
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::Modal(service) => service.embed_batch(texts).await,
            Self::Direct(service) => service.embed_batch(texts).await,
        }
    }

    /// Generate embeddings with automatic chunking
    pub async fn embed_batch_chunked(
        &self,
        texts: Vec<String>,
        chunk_size: usize,
    ) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::Modal(service) => service.embed_batch_chunked(texts, Some(chunk_size)).await,
            Self::Direct(service) => service.embed_batch_chunked(texts, chunk_size).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modal_request_serialization() {
        let request = ModalEmbeddingRequest {
            texts: vec!["test".to_string()],
            model: Some("my-model".to_string()),
        };

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        assert!(json.contains("test"));
        assert!(json.contains("my-model"));
    }
}
