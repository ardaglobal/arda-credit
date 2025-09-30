use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info};

use crate::services::magic_link_service::MagicLinkService;

/// Background service to clean up expired magic links
pub struct MagicLinkCleanupService {
    magic_link_service: Arc<MagicLinkService>,
    cleanup_interval: Duration,
}

impl MagicLinkCleanupService {
    pub fn new(magic_link_service: Arc<MagicLinkService>) -> Self {
        Self {
            magic_link_service,
            cleanup_interval: Duration::from_secs(3600), // Run every hour
        }
    }

    /// Start the cleanup background task
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let magic_link_service = self.magic_link_service.clone();
        let interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);

            info!(
                "🧹 Magic link cleanup service started (interval: {}s)",
                interval.as_secs()
            );

            loop {
                interval_timer.tick().await;

                match magic_link_service.cleanup_expired_magic_links().await {
                    Ok(count) => {
                        if count > 0 {
                            debug!("Cleaned up {} expired magic links", count);
                        }
                    }
                    Err(e) => {
                        error!("Failed to cleanup expired magic links: {}", e);
                    }
                }
            }
        })
    }
}
