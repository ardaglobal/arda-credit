use anyhow::{Context, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use super::surrealdb_service::SurrealDBService;
use super::Document;

/// Configuration for the job queue worker
#[derive(Debug, Clone)]
pub struct JobQueueConfig {
    /// How often to poll for new documents (in seconds)
    pub poll_interval_secs: u64,
    /// Maximum number of concurrent processing tasks
    pub max_concurrent_jobs: usize,
    /// Maximum number of retry attempts per document
    pub max_retries: u32,
    /// Initial backoff duration for retries (in milliseconds)
    pub initial_backoff_ms: u64,
    /// Maximum backoff duration for retries (in milliseconds)
    pub max_backoff_ms: u64,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Batch size for fetching documents
    pub fetch_batch_size: usize,
}

impl Default for JobQueueConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 5,
            max_concurrent_jobs: 4,
            max_retries: 3,
            initial_backoff_ms: 1000,
            max_backoff_ms: 60000,
            backoff_multiplier: 2.0,
            fetch_batch_size: 10,
        }
    }
}

impl JobQueueConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            poll_interval_secs: std::env::var("JOB_QUEUE_POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            max_concurrent_jobs: std::env::var("JOB_QUEUE_MAX_CONCURRENT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4),
            max_retries: std::env::var("JOB_QUEUE_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            initial_backoff_ms: std::env::var("JOB_QUEUE_INITIAL_BACKOFF_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            max_backoff_ms: std::env::var("JOB_QUEUE_MAX_BACKOFF_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60000),
            backoff_multiplier: std::env::var("JOB_QUEUE_BACKOFF_MULTIPLIER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2.0),
            fetch_batch_size: std::env::var("JOB_QUEUE_FETCH_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
        }
    }
}

/// Job processing handler trait
///
/// Implement this trait to define custom document processing logic
#[async_trait::async_trait]
pub trait JobProcessor: Send + Sync {
    /// Process a single document
    ///
    /// # Arguments
    /// * `document` - The document to process
    ///
    /// # Returns
    /// * Ok(()) if processing succeeded
    /// * Err(_) if processing failed (will trigger retry logic)
    async fn process_document(&self, document: &Document) -> Result<()>;
}

/// Job queue worker that processes documents in the background
pub struct JobQueueWorker {
    db: Arc<SurrealDBService>,
    config: JobQueueConfig,
    processor: Arc<dyn JobProcessor>,
    semaphore: Arc<Semaphore>,
    shutdown_tx: Option<tokio::sync::mpsc::Sender<()>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl JobQueueWorker {
    /// Create a new job queue worker
    ///
    /// # Arguments
    /// * `db` - SurrealDB service instance
    /// * `config` - Worker configuration
    /// * `processor` - Document processing implementation
    pub fn new(
        db: Arc<SurrealDBService>,
        config: JobQueueConfig,
        processor: Arc<dyn JobProcessor>,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_jobs));

        info!(
            "Initializing job queue worker (poll: {}s, max_concurrent: {}, max_retries: {})",
            config.poll_interval_secs, config.max_concurrent_jobs, config.max_retries
        );

        Self {
            db,
            config,
            processor,
            semaphore,
            shutdown_tx: None,
            worker_handle: None,
        }
    }

    /// Start the worker (spawns a background task)
    ///
    /// # Returns
    /// * Shutdown sender channel (send () to stop the worker)
    pub fn start(&mut self) -> tokio::sync::mpsc::Sender<()> {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);

        let db = Arc::clone(&self.db);
        let config = self.config.clone();
        let processor = Arc::clone(&self.processor);
        let semaphore = Arc::clone(&self.semaphore);

        let handle = tokio::spawn(async move {
            info!("Job queue worker started");

            loop {
                // Check for shutdown signal
                if shutdown_rx.try_recv().is_ok() {
                    info!("Job queue worker received shutdown signal");
                    break;
                }

                // Poll for pending documents
                match Self::poll_and_process(&db, &config, &processor, &semaphore).await {
                    Ok(processed_count) => {
                        if processed_count > 0 {
                            debug!("Processed {} documents in this cycle", processed_count);
                        }
                    }
                    Err(e) => {
                        error!("Error in job queue worker cycle: {}", e);
                    }
                }

                // Wait before next poll
                tokio::time::sleep(Duration::from_secs(config.poll_interval_secs)).await;
            }

            info!("Job queue worker stopped");
        });

        self.shutdown_tx = Some(shutdown_tx.clone());
        self.worker_handle = Some(handle);

        shutdown_tx
    }

    /// Stop the worker gracefully
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            info!("Stopping job queue worker...");
            tx.send(()).await.ok();

            if let Some(handle) = self.worker_handle.take() {
                handle.await.context("Failed to join worker task")?;
            }

            info!("Job queue worker stopped successfully");
        }

        Ok(())
    }

    /// Poll for pending documents and process them
    async fn poll_and_process(
        db: &Arc<SurrealDBService>,
        config: &JobQueueConfig,
        processor: &Arc<dyn JobProcessor>,
        semaphore: &Arc<Semaphore>,
    ) -> Result<usize> {
        // Fetch documents with status 'uploaded' or 'queued'
        let uploaded_docs = db
            .get_documents_by_status("uploaded", config.fetch_batch_size)
            .await?;
        let queued_docs = db
            .get_documents_by_status("queued", config.fetch_batch_size)
            .await?;

        let all_docs: Vec<Document> = uploaded_docs
            .into_iter()
            .chain(queued_docs.into_iter())
            .take(config.fetch_batch_size)
            .collect();

        if all_docs.is_empty() {
            return Ok(0);
        }

        info!("Found {} documents to process", all_docs.len());

        // Process documents concurrently
        let mut tasks = Vec::new();

        for document in all_docs {
            info!(
                "📋 Spawning processing task for document {} ({})",
                document.document_id, document.filename
            );

            let db = Arc::clone(db);
            let processor = Arc::clone(processor);
            let semaphore = Arc::clone(semaphore);
            let config = config.clone();

            // Acquire semaphore permit
            info!(
                "🎫 Acquiring semaphore permit for document {}",
                document.document_id
            );
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            info!(
                "✅ Semaphore permit acquired for document {}",
                document.document_id
            );

            let task = tokio::spawn(async move {
                info!("🚀 Task started for document {}", document.document_id);
                let result =
                    Self::process_single_document(&db, &document, &processor, &config).await;

                info!(
                    "📤 Task completed for document {} with result: {:?}",
                    document.document_id,
                    result.as_ref().map(|_| "Ok").map_err(|e| e.to_string())
                );

                // Release permit
                drop(permit);

                result
            });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut processed_count = 0;
        for task in tasks {
            match task.await {
                Ok(Ok(())) => processed_count += 1,
                Ok(Err(e)) => error!("Document processing failed: {}", e),
                Err(e) => error!("Task join error: {}", e),
            }
        }

        Ok(processed_count)
    }

    /// Process a single document with retry logic
    async fn process_single_document(
        db: &Arc<SurrealDBService>,
        document: &Document,
        processor: &Arc<dyn JobProcessor>,
        config: &JobQueueConfig,
    ) -> Result<()> {
        let document_id = document.document_id;

        info!(
            "🔄 Attempting to claim document {} ({}) for processing",
            document_id, document.filename
        );

        // Atomically claim document for processing
        let claimed = db.claim_document_for_processing(document_id).await?;
        if !claimed {
            info!(
                "⏭️  Document {} already being processed, skipping",
                document_id
            );
            return Ok(());
        }

        info!(
            "✅ Claimed document {} ({}), starting processing",
            document_id, document.filename
        );

        // Process with retry logic
        let mut attempt = 0;
        let mut backoff = Duration::from_millis(config.initial_backoff_ms);

        loop {
            attempt += 1;

            // Update progress at start of attempt
            if attempt > 1 {
                db.update_document_status(
                    document_id,
                    "processing",
                    Some(0.1), // Reset progress on retry
                    Some(format!("Retry attempt {}/{}", attempt, config.max_retries)),
                )
                .await
                .ok();
            }

            // Attempt processing
            match processor.process_document(document).await {
                Ok(()) => {
                    // Success - mark as completed
                    db.update_document_status(document_id, "completed", Some(1.0), None)
                        .await?;
                    info!(
                        "Successfully processed document {} (attempts: {})",
                        document_id, attempt
                    );
                    return Ok(());
                }
                Err(e) => {
                    let error_message = format!("Processing error: {}", e);
                    warn!(
                        "Document {} processing failed (attempt {}/{}): {}",
                        document_id, attempt, config.max_retries, error_message
                    );

                    // Check if we should retry
                    if attempt >= config.max_retries {
                        // Max retries reached - mark as failed
                        db.increment_retry_count(document_id).await.ok();
                        db.update_document_status(
                            document_id,
                            "failed",
                            Some(0.0),
                            Some(error_message),
                        )
                        .await?;
                        error!(
                            "Document {} processing failed after {} attempts",
                            document_id, config.max_retries
                        );
                        return Err(anyhow::anyhow!(
                            "Processing failed after {} attempts",
                            config.max_retries
                        ));
                    }

                    // Increment retry count
                    db.increment_retry_count(document_id).await.ok();

                    // Exponential backoff
                    debug!("Retrying document {} after {:?}...", document_id, backoff);
                    tokio::time::sleep(backoff).await;
                    backoff = std::cmp::min(
                        Duration::from_secs_f64(backoff.as_secs_f64() * config.backoff_multiplier),
                        Duration::from_millis(config.max_backoff_ms),
                    );
                }
            }
        }
    }
}

impl Drop for JobQueueWorker {
    fn drop(&mut self) {
        if self.shutdown_tx.is_some() {
            warn!(
                "JobQueueWorker dropped without being stopped - use stop() for graceful shutdown"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = JobQueueConfig::default();
        assert_eq!(config.poll_interval_secs, 5);
        assert_eq!(config.max_concurrent_jobs, 4);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_backoff_calculation() {
        let config = JobQueueConfig::default();
        let mut backoff = Duration::from_millis(config.initial_backoff_ms);

        // First retry: 1000ms
        assert_eq!(backoff.as_millis(), 1000);

        // Second retry: 2000ms
        backoff = Duration::from_secs_f64(backoff.as_secs_f64() * config.backoff_multiplier);
        assert_eq!(backoff.as_millis(), 2000);

        // Third retry: 4000ms
        backoff = Duration::from_secs_f64(backoff.as_secs_f64() * config.backoff_multiplier);
        assert_eq!(backoff.as_millis(), 4000);
    }
}
