use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info};

use crate::session_service::SessionService;

/// Background service to clean up expired sessions
pub struct SessionCleanupService {
    session_service: Arc<SessionService>,
    cleanup_interval: Duration,
}

impl SessionCleanupService {
    pub fn new(session_service: Arc<SessionService>) -> Self {
        Self {
            session_service,
            cleanup_interval: Duration::from_secs(3600), // Run every hour
        }
    }

    /// Start the cleanup background task
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let session_service = self.session_service.clone();
        let interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut interval_timer = time::interval(interval);

            info!(
                "🧹 Session cleanup service started (interval: {}s)",
                interval.as_secs()
            );

            loop {
                interval_timer.tick().await;

                match session_service.cleanup_expired_sessions().await {
                    Ok(count) => {
                        if count > 0 {
                            debug!("Cleaned up {} expired sessions", count);
                        }
                    }
                    Err(e) => {
                        error!("Failed to cleanup expired sessions: {}", e);
                    }
                }
            }
        })
    }
}
