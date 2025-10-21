use anyhow::{Context, Result};
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Embedding provider
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EmbeddingProvider {
    /// OpenAI API
    OpenAI,
    /// Modal custom endpoint
    Modal,
}

/// Embedding models
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingModel {
    /// text-embedding-3-small (1536 dimensions, $0.02/1M tokens)
    TextEmbedding3Small,
    /// text-embedding-3-large (3072 dimensions, $0.13/1M tokens)
    TextEmbedding3Large,
    /// text-embedding-ada-002 (1536 dimensions, legacy)
    Ada002,
    /// Qwen3-Embedding-8B (4096 dimensions, hosted on Modal TEI)
    Qwen3Embedding8B,
}

impl EmbeddingModel {
    pub fn as_str(&self) -> &str {
        match self {
            EmbeddingModel::TextEmbedding3Small => "text-embedding-3-small",
            EmbeddingModel::TextEmbedding3Large => "text-embedding-3-large",
            EmbeddingModel::Ada002 => "text-embedding-ada-002",
            EmbeddingModel::Qwen3Embedding8B => "qwen3-embedding-8b",
        }
    }

    pub fn dimensions(&self) -> usize {
        match self {
            EmbeddingModel::TextEmbedding3Small => 1536,
            EmbeddingModel::TextEmbedding3Large => 3072,
            EmbeddingModel::Ada002 => 1536,
            EmbeddingModel::Qwen3Embedding8B => 4096,
        }
    }

    /// Parse embedding model from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "text-embedding-3-small" => Some(EmbeddingModel::TextEmbedding3Small),
            "text-embedding-3-large" => Some(EmbeddingModel::TextEmbedding3Large),
            "text-embedding-ada-002" => Some(EmbeddingModel::Ada002),
            "qwen3-embedding-8b" | "qwen3" => Some(EmbeddingModel::Qwen3Embedding8B),
            _ => None,
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

/// Retry configuration for failed requests
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial backoff duration
    pub initial_backoff: Duration,
    /// Maximum backoff duration
    pub max_backoff: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Regular request timeout (for warm containers)
    pub timeout: Duration,
    /// Warmup request timeout (for cold starts, longer)
    pub warmup_timeout: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 5,                           // Increased from 3 to 5
            initial_backoff: Duration::from_secs(15), // Fixed 15s for first retry
            max_backoff: Duration::from_secs(120),    // Max 120s
            backoff_multiplier: 2.0,
            timeout: Duration::from_secs(120), // Regular timeout
            warmup_timeout: Duration::from_secs(180), // Warmup timeout for cold starts
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum concurrent requests
    pub max_concurrent_requests: usize,
    /// Minimum delay between requests
    pub min_request_delay: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 10,
            min_request_delay: Duration::from_millis(100),
        }
    }
}

/// Embedding generation metrics
#[derive(Debug, Clone, Default)]
pub struct EmbeddingMetrics {
    pub total_requests: usize,
    pub successful_requests: usize,
    pub failed_requests: usize,
    pub total_chunks: usize,
    pub total_duration: Duration,
    pub retries: usize,
}

impl EmbeddingMetrics {
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_requests as f64 / self.total_requests as f64
        }
    }

    pub fn avg_time_per_chunk(&self) -> Duration {
        if self.total_chunks == 0 {
            Duration::ZERO
        } else {
            self.total_duration / self.total_chunks as u32
        }
    }
}

/// Embedding Service for converting text to vectors
///
/// Supports multiple providers:
/// - OpenAI API
/// - Modal custom endpoint (QWen3B hosted model)
///
/// Features:
/// - Retry logic with exponential backoff
/// - Rate limiting
/// - Embedding validation
/// - In-memory caching
/// - Metrics tracking
pub struct EmbeddingService {
    client: Client,
    provider: EmbeddingProvider,
    api_key: Option<String>,
    model: EmbeddingModel,
    base_url: String,
    retry_config: RetryConfig,
    rate_limiter: Arc<Semaphore>,
    rate_limit_config: RateLimitConfig,
    last_request_time: Arc<tokio::sync::Mutex<Instant>>,
    /// In-memory cache: text -> embedding vector
    embedding_cache: Arc<DashMap<String, Vec<f32>>>,
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

        // Default model based on provider
        let default_model = match provider {
            EmbeddingProvider::OpenAI => "text-embedding-3-small",
            EmbeddingProvider::Modal => "qwen3-embedding-8b",
        };

        let model_str =
            std::env::var("EMBEDDING_MODEL").unwrap_or_else(|_| default_model.to_string());

        let model = EmbeddingModel::from_str(&model_str).unwrap_or_else(|| {
            warn!(
                "Unknown embedding model '{}', defaulting to {}",
                model_str, default_model
            );
            EmbeddingModel::from_str(default_model).unwrap()
        });

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
        Self::new_with_config(
            provider,
            api_key,
            model,
            base_url,
            RetryConfig::default(),
            RateLimitConfig::default(),
        )
    }

    /// Create a new embedding service with custom retry and rate limit configuration
    pub fn new_with_config(
        provider: EmbeddingProvider,
        api_key: Option<String>,
        model: EmbeddingModel,
        base_url: String,
        retry_config: RetryConfig,
        rate_limit_config: RateLimitConfig,
    ) -> Result<Self> {
        // Use the warmup timeout as the client timeout (longest possible wait)
        let client = Client::builder()
            .timeout(retry_config.warmup_timeout)
            .build()?;

        let rate_limiter = Arc::new(Semaphore::new(rate_limit_config.max_concurrent_requests));

        Ok(Self {
            client,
            provider,
            api_key,
            model,
            base_url,
            retry_config,
            rate_limiter,
            rate_limit_config,
            last_request_time: Arc::new(tokio::sync::Mutex::new(Instant::now())),
            embedding_cache: Arc::new(DashMap::new()),
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
    /// Uses in-memory cache to avoid re-requesting embeddings for duplicate texts.
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

        // Check cache for existing embeddings
        let mut cached_embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(texts.len());
        let mut texts_to_fetch = Vec::new();
        let mut fetch_indices = Vec::new();

        for (i, text) in texts.iter().enumerate() {
            if let Some(embedding) = self.embedding_cache.get(text) {
                cached_embeddings.push(Some(embedding.clone()));
            } else {
                cached_embeddings.push(None);
                texts_to_fetch.push(text.clone());
                fetch_indices.push(i);
            }
        }

        let cache_hits = texts.len() - texts_to_fetch.len();
        if cache_hits > 0 {
            debug!(
                "Cache hits: {}/{} ({:.1}%)",
                cache_hits,
                texts.len(),
                (cache_hits as f64 / texts.len() as f64) * 100.0
            );
        }

        // Fetch embeddings for non-cached texts
        let fetched_embeddings = if !texts_to_fetch.is_empty() {
            let embeddings = match self.provider {
                EmbeddingProvider::OpenAI => {
                    self.embed_batch_openai(texts_to_fetch.clone()).await?
                }
                EmbeddingProvider::Modal => self.embed_batch_modal(texts_to_fetch.clone()).await?,
            };

            // Store in cache
            for (text, embedding) in texts_to_fetch.iter().zip(embeddings.iter()) {
                self.embedding_cache.insert(text.clone(), embedding.clone());
            }

            embeddings
        } else {
            Vec::new()
        };

        // Combine cached and fetched embeddings in original order
        let mut result = Vec::with_capacity(texts.len());
        let mut fetch_iter = fetched_embeddings.into_iter();

        for cached in cached_embeddings {
            if let Some(embedding) = cached {
                result.push(embedding);
            } else {
                result.push(fetch_iter.next().unwrap());
            }
        }

        Ok(result)
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> (usize, usize) {
        let size = self.embedding_cache.len();
        // Estimate memory usage: each embedding is ~1536 * 4 bytes + overhead
        let estimated_bytes = size * (self.dimensions() * 4 + 50);
        (size, estimated_bytes)
    }

    /// Clear the embedding cache
    pub fn clear_cache(&self) {
        let size = self.embedding_cache.len();
        self.embedding_cache.clear();
        info!("Cleared embedding cache ({} entries)", size);
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

    /// Generate embeddings using Modal endpoint with retry logic and validation
    async fn embed_batch_modal(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let start_time = Instant::now();
        let num_texts = texts.len();

        // Acquire rate limit permit
        let _permit = self.rate_limiter.acquire().await.unwrap();

        // Enforce minimum delay between requests
        {
            let mut last_time = self.last_request_time.lock().await;
            let elapsed = last_time.elapsed();
            if elapsed < self.rate_limit_config.min_request_delay {
                let sleep_duration = self.rate_limit_config.min_request_delay - elapsed;
                tokio::time::sleep(sleep_duration).await;
            }
            *last_time = Instant::now();
        }

        // Retry with exponential backoff
        let embeddings = self.embed_batch_modal_with_retry(texts.clone()).await?;

        // Validate embeddings
        self.validate_embeddings(&embeddings, num_texts)?;

        let duration = start_time.elapsed();
        debug!(
            "Modal embedding completed: {} texts in {:.2}s ({:.0} ms/text)",
            num_texts,
            duration.as_secs_f64(),
            duration.as_millis() as f64 / num_texts as f64
        );

        Ok(embeddings)
    }

    /// Generate embeddings using Modal endpoint with retry logic
    async fn embed_batch_modal_with_retry(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        // Modal endpoint expects TEI-compatible format
        #[derive(Serialize)]
        struct ModalRequest {
            texts: Vec<String>,
            model: String,
        }

        // TEI response format from Modal
        #[derive(Deserialize, Debug)]
        struct ModalEmbeddingResult {
            embedding: Vec<f32>,
            #[allow(dead_code)]
            model: Option<String>,
            #[allow(dead_code)]
            dimensions: Option<usize>,
            success: bool,
            #[allow(dead_code)]
            backend: Option<String>,
            error_message: Option<String>,
        }

        #[derive(Deserialize, Debug)]
        struct ModalResponse {
            results: Vec<ModalEmbeddingResult>,
        }

        let request = ModalRequest {
            texts: texts.clone(),
            model: self.model.as_str().to_string(),
        };

        // Call Modal endpoint
        let url = if self.base_url.ends_with('/') {
            format!("{}embed", self.base_url)
        } else {
            format!("{}/embed", self.base_url)
        };

        let mut attempt = 0;
        let mut backoff = self.retry_config.initial_backoff;

        loop {
            attempt += 1;

            let mut req = self.client.post(&url).json(&request);

            // Add auth header if API key is provided
            if let Some(api_key) = &self.api_key {
                req = req.header("Authorization", format!("Bearer {}", api_key));
            }

            match req.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        // Parse response
                        match response.json::<ModalResponse>().await {
                            Ok(modal_response) => {
                                if attempt > 1 {
                                    info!("Modal request succeeded after {} attempts", attempt);
                                }

                                // Extract embeddings from results array
                                let mut embeddings = Vec::new();
                                for (i, result) in modal_response.results.iter().enumerate() {
                                    if !result.success {
                                        let error_msg = result
                                            .error_message
                                            .as_ref()
                                            .map(|s| s.as_str())
                                            .unwrap_or("Unknown error");
                                        error!("Embedding {} failed: {}", i, error_msg);
                                        return Err(anyhow::anyhow!(
                                            "Embedding generation failed: {}",
                                            error_msg
                                        ));
                                    }

                                    // Validate and clean embedding (replace None/NaN with 0.0)
                                    let cleaned_embedding = self.clean_embedding(&result.embedding);
                                    embeddings.push(cleaned_embedding);
                                }

                                return Ok(embeddings);
                            }
                            Err(e) => {
                                error!("Failed to parse Modal response: {}", e);
                                if attempt >= self.retry_config.max_retries {
                                    return Err(anyhow::anyhow!(
                                        "Failed to parse Modal response after {} attempts: {}",
                                        attempt,
                                        e
                                    ));
                                }
                            }
                        }
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_default();

                        // Check if error is retryable
                        let is_retryable = status.is_server_error()
                            || status == reqwest::StatusCode::TOO_MANY_REQUESTS;

                        if !is_retryable || attempt >= self.retry_config.max_retries {
                            return Err(anyhow::anyhow!(
                                "Modal API error ({}) after {} attempts: {}",
                                status,
                                attempt,
                                error_text
                            ));
                        }

                        warn!(
                            "Modal request failed with status {} (attempt {}/{}): {}",
                            status, attempt, self.retry_config.max_retries, error_text
                        );
                    }
                }
                Err(e) => {
                    if attempt >= self.retry_config.max_retries {
                        return Err(anyhow::anyhow!(
                            "Failed to send request to Modal after {} attempts: {}",
                            attempt,
                            e
                        ));
                    }
                    warn!(
                        "Modal request failed (attempt {}/{}): {}",
                        attempt, self.retry_config.max_retries, e
                    );
                }
            }

            // Exponential backoff
            if attempt < self.retry_config.max_retries {
                debug!("Retrying in {:?}...", backoff);
                tokio::time::sleep(backoff).await;
                backoff = std::cmp::min(
                    Duration::from_secs_f64(
                        backoff.as_secs_f64() * self.retry_config.backoff_multiplier,
                    ),
                    self.retry_config.max_backoff,
                );
            }
        }
    }

    /// Clean embedding by replacing invalid values (NaN, infinite) with 0.0
    /// This is more resilient than rejecting the entire batch
    fn clean_embedding(&self, embedding: &[f32]) -> Vec<f32> {
        embedding
            .iter()
            .map(|&value| {
                if value.is_finite() {
                    value
                } else {
                    // Replace NaN or infinite with 0.0
                    0.0
                }
            })
            .collect()
    }

    /// Validate embedding dimensions and values
    fn validate_embeddings(&self, embeddings: &[Vec<f32>], expected_count: usize) -> Result<()> {
        // Check count matches
        if embeddings.len() != expected_count {
            anyhow::bail!(
                "Embedding count mismatch: expected {}, got {}",
                expected_count,
                embeddings.len()
            );
        }

        let expected_dim = self.dimensions();

        for (i, embedding) in embeddings.iter().enumerate() {
            // Check dimensions
            if embedding.len() != expected_dim {
                anyhow::bail!(
                    "Embedding {} has wrong dimensions: expected {}, got {}",
                    i,
                    expected_dim,
                    embedding.len()
                );
            }

            // Check for NaN or infinite values
            for (j, &value) in embedding.iter().enumerate() {
                if !value.is_finite() {
                    anyhow::bail!(
                        "Embedding {} has invalid value at index {}: {}",
                        i,
                        j,
                        value
                    );
                }
            }

            // Check if embedding is not all zeros (sanity check)
            let sum: f32 = embedding.iter().sum();
            if sum.abs() < 1e-8 {
                warn!("Embedding {} appears to be all zeros", i);
            }
        }

        debug!(
            "Validated {} embeddings (dim={})",
            embeddings.len(),
            expected_dim
        );
        Ok(())
    }

    /// Generate embeddings in chunks to handle large batches
    ///
    /// Automatically splits large batches into multiple API calls
    /// with optimal chunk size for throughput.
    ///
    /// # Arguments
    /// * `texts` - Vector of texts to embed
    /// * `chunk_size` - Size of each batch (default: 100 for OpenAI, 50 for Modal)
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

        let start_time = Instant::now();

        // Optimize chunk size based on provider
        let optimal_chunk_size = match self.provider {
            EmbeddingProvider::Modal => chunk_size.min(50), // Modal QWen3B works best with smaller batches
            EmbeddingProvider::OpenAI => chunk_size.min(100),
        };

        info!(
            "Generating embeddings for {} texts in chunks of {} (provider: {:?})",
            texts.len(),
            optimal_chunk_size,
            self.provider
        );

        let mut all_embeddings = Vec::with_capacity(texts.len());
        let total_chunks = (texts.len() + optimal_chunk_size - 1) / optimal_chunk_size;

        for (i, chunk) in texts.chunks(optimal_chunk_size).enumerate() {
            debug!(
                "Processing chunk {}/{} ({} texts)",
                i + 1,
                total_chunks,
                chunk.len()
            );

            let chunk_embeddings = self.embed_batch(chunk.to_vec()).await?;
            all_embeddings.extend(chunk_embeddings);

            // Progress logging
            if (i + 1) % 10 == 0 || i + 1 == total_chunks {
                let progress = (i + 1) as f64 / total_chunks as f64 * 100.0;
                let elapsed = start_time.elapsed();
                let rate = all_embeddings.len() as f64 / elapsed.as_secs_f64();
                info!(
                    "Progress: {:.1}% ({}/{} chunks, {:.0} embeddings/sec)",
                    progress,
                    i + 1,
                    total_chunks,
                    rate
                );
            }
        }

        let total_duration = start_time.elapsed();
        info!(
            "Completed embedding generation: {} texts in {:.2}s ({:.0} texts/sec)",
            texts.len(),
            total_duration.as_secs_f64(),
            texts.len() as f64 / total_duration.as_secs_f64()
        );

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

    /// Pre-warm multiple Modal containers before ingestion
    ///
    /// This is particularly useful for Modal deployments where containers
    /// may be cold and need time to spin up. Uses parallel warmup with
    /// graceful degradation.
    ///
    /// # Arguments
    /// * `num_containers` - Number of containers to pre-warm (default: 4)
    /// * `min_success_rate` - Minimum fraction of containers that must succeed (0.0-1.0, default: 0.5)
    ///
    /// # Returns
    /// * `true` if at least min_success_rate containers warmed up successfully
    pub async fn warmup_containers(
        &self,
        num_containers: usize,
        min_success_rate: f64,
    ) -> Result<bool> {
        info!(
            "🔥 Warming up {} Modal containers (model: {}, dim: {})...",
            num_containers,
            self.model.as_str(),
            self.dimensions()
        );
        info!("   Cold start may take 30-60s (connection resets are normal)");

        // Create warmup tasks
        let mut tasks = Vec::new();
        for i in 0..num_containers {
            let service = self.clone_for_warmup();
            let task = tokio::spawn(async move { service.send_warmup_request(i + 1).await });
            tasks.push(task);
        }

        // Wait for all warmup requests to complete
        let mut results = Vec::new();
        for (i, task) in tasks.into_iter().enumerate() {
            match task.await {
                Ok(Ok(success)) => {
                    results.push(success);
                    if success {
                        info!("✅ Container {}/{} ready", i + 1, num_containers);
                    } else {
                        warn!("⚠️ Container {}/{} warmup failed", i + 1, num_containers);
                    }
                }
                Ok(Err(e)) => {
                    error!("❌ Container {}/{} error: {}", i + 1, num_containers, e);
                    results.push(false);
                }
                Err(e) => {
                    error!(
                        "❌ Container {}/{} join error: {}",
                        i + 1,
                        num_containers,
                        e
                    );
                    results.push(false);
                }
            }
        }

        let successful = results.iter().filter(|&&r| r).count();
        let required = (num_containers as f64 * min_success_rate).ceil() as usize;

        info!("\n🔥 Warmup Results:");
        info!("   ✅ Successful: {}/{}", successful, num_containers);
        info!(
            "   📊 Required: {}/{} ({:.0}%)",
            required,
            num_containers,
            min_success_rate * 100.0
        );

        if successful >= required {
            info!(
                "✅ {} Modal container(s) ready for embedding generation!",
                successful
            );
            if successful < num_containers {
                warn!(
                    "   ⚠️ Running with reduced capacity ({}/{} containers)",
                    successful, num_containers
                );
            }
            Ok(true)
        } else {
            error!(
                "❌ Insufficient containers ready: {}/{} (need {})",
                successful, num_containers, required
            );
            error!("   Modal service may be cold starting or unavailable");
            Ok(false)
        }
    }

    /// Clone service for warmup (lightweight clone for concurrent warmup)
    fn clone_for_warmup(&self) -> Self {
        Self {
            client: self.client.clone(),
            provider: self.provider,
            api_key: self.api_key.clone(),
            model: self.model,
            base_url: self.base_url.clone(),
            retry_config: self.retry_config.clone(),
            rate_limiter: Arc::clone(&self.rate_limiter),
            rate_limit_config: self.rate_limit_config.clone(),
            last_request_time: Arc::clone(&self.last_request_time),
            embedding_cache: Arc::clone(&self.embedding_cache),
        }
    }

    /// Send a single warmup request to trigger container spin-up
    async fn send_warmup_request(&self, container_num: usize) -> Result<bool> {
        let warmup_text = vec![format!("warmup request {}", container_num)];

        // Use shorter timeout for warmup health check
        let start = Instant::now();
        match tokio::time::timeout(
            self.retry_config.warmup_timeout,
            self.embed_batch(warmup_text),
        )
        .await
        {
            Ok(Ok(embeddings)) => {
                let duration = start.elapsed();
                debug!(
                    "Container {} ready in {:.2}s",
                    container_num,
                    duration.as_secs_f64()
                );
                Ok(!embeddings.is_empty())
            }
            Ok(Err(e)) => {
                // Service responding but embedding failed - log as warning
                if e.to_string().contains("Connection reset")
                    || e.to_string().contains("Connection aborted")
                {
                    warn!(
                        "🔌 Connection reset for container {} (common during cold start)",
                        container_num
                    );
                } else {
                    warn!(
                        "⚠️ Warmup request {} failed: {}",
                        container_num,
                        e.to_string().chars().take(100).collect::<String>()
                    );
                }
                Ok(false)
            }
            Err(_) => {
                warn!(
                    "⏰ Warmup request {} timed out (waited {}s)",
                    container_num,
                    self.retry_config.warmup_timeout.as_secs()
                );
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
        assert_eq!(EmbeddingModel::Qwen3Embedding8B.dimensions(), 4096);
    }

    #[test]
    fn test_embedding_model_from_str() {
        assert_eq!(
            EmbeddingModel::from_str("qwen3-embedding-8b"),
            Some(EmbeddingModel::Qwen3Embedding8B)
        );
        assert_eq!(
            EmbeddingModel::from_str("qwen3"),
            Some(EmbeddingModel::Qwen3Embedding8B)
        );
        assert_eq!(
            EmbeddingModel::from_str("text-embedding-3-small"),
            Some(EmbeddingModel::TextEmbedding3Small)
        );
        assert_eq!(EmbeddingModel::from_str("unknown"), None);
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
