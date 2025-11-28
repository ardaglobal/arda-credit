//! Collateral Duplicate Checker Background Service
//!
//! Processes the queue of pending collateral duplicate checks in the background.
//! Runs continuously on server startup and picks up failed checks for retry.

use arda_credit_db::CollateralCheckQueueEntry;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info, warn};

/// Background service for processing collateral duplicate checks
pub struct CollateralDuplicateChecker {
    pool: PgPool,
    check_interval: Duration,
    max_retries: i32,
}

impl CollateralDuplicateChecker {
    /// Create a new collateral duplicate checker service
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            check_interval: Duration::from_secs(30), // Check every 30 seconds
            max_retries: 3,
        }
    }

    /// Start the background queue processor
    ///
    /// This spawns a tokio task that runs continuously, checking for pending
    /// duplicate checks and processing them.
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let pool = self.pool.clone();
        let interval = self.check_interval;
        let max_retries = self.max_retries;

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);

            info!(
                "🔍 Collateral duplicate checker started (interval: {}s)",
                interval.as_secs()
            );

            loop {
                interval_timer.tick().await;

                // Process pending and failed checks
                if let Err(e) = Self::process_queue(&pool, max_retries).await {
                    error!("Failed to process duplicate check queue: {}", e);
                }
            }
        })
    }

    /// Process items from the queue
    ///
    /// Uses FOR UPDATE SKIP LOCKED to claim the next item without race conditions.
    async fn process_queue(
        pool: &PgPool,
        max_retries: i32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Claim next pending/failed item (with row-level locking to prevent race)
        let item = sqlx::query_as!(
            CollateralCheckQueueEntry,
            r#"
            UPDATE collateral_check_queue
            SET status = 'processing', started_at = NOW()
            WHERE id = (
                SELECT id FROM collateral_check_queue
                WHERE (status = 'pending' OR (status = 'failed' AND retry_count < $1))
                ORDER BY priority DESC, created_at ASC
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            )
            RETURNING id, collateral_id, status as "status!", priority as "priority!", 
                      retry_count as "retry_count!", last_error,
                      created_at as "created_at!", started_at, completed_at
            "#,
            max_retries
        )
        .fetch_optional(pool)
        .await?;

        if let Some(entry) = item {
            debug!(
                "Processing duplicate check for collateral {}",
                entry.collateral_id
            );

            // Process the duplicate check
            match arda_credit_db::process_duplicate_check(pool, entry.collateral_id).await {
                Ok(_) => {
                    // Mark as completed
                    sqlx::query!(
                        r#"
                        UPDATE collateral_check_queue
                        SET status = 'completed', completed_at = NOW()
                        WHERE id = $1
                        "#,
                        entry.id
                    )
                    .execute(pool)
                    .await?;

                    debug!(
                        "✅ Duplicate check completed for collateral {}",
                        entry.collateral_id
                    );
                }
                Err(e) => {
                    // Mark as failed and increment retry count
                    let error_msg = e.to_string();
                    let new_retry_count = entry.retry_count + 1;

                    sqlx::query!(
                        r#"
                        UPDATE collateral_check_queue
                        SET status = 'failed', retry_count = $1, last_error = $2
                        WHERE id = $3
                        "#,
                        new_retry_count,
                        error_msg,
                        entry.id
                    )
                    .execute(pool)
                    .await?;

                    warn!(
                        "❌ Duplicate check failed for collateral {} (attempt {}/{}): {}",
                        entry.collateral_id, new_retry_count, max_retries, error_msg
                    );
                }
            }
        }

        Ok(())
    }
}
