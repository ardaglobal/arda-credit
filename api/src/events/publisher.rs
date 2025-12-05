//! Event publisher for collateral intelligence service
//!
//! Provides asynchronous, fire-and-forget event publishing with retry logic.

use super::types::{CollateralEvent, DealEvent};
use reqwest::Client;
use std::time::Duration;
use tracing::{info, warn};

/// Publisher for collateral and deal events
#[derive(Clone)]
pub struct CollateralEventPublisher {
    intel_service_url: String,
    client: Client,
}

impl CollateralEventPublisher {
    /// Create a new event publisher
    pub fn new(intel_service_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client for event publisher");

        Self {
            intel_service_url,
            client,
        }
    }

    /// Publish a collateral event with retry logic
    pub async fn publish_collateral(&self, event: CollateralEvent) -> Result<(), PublishError> {
        let url = format!("{}/api/v1/events/collateral", self.intel_service_url);
        let event_id = event.event_id();

        self.publish_with_retry(&url, &event, event_id, "collateral")
            .await
    }

    /// Publish a deal event with retry logic
    pub async fn publish_deal(&self, event: DealEvent) -> Result<(), PublishError> {
        let url = format!("{}/api/v1/events/deal", self.intel_service_url);
        let event_id = event.event_id();

        self.publish_with_retry(&url, &event, event_id, "deal")
            .await
    }

    /// Internal method to publish with exponential backoff retry
    async fn publish_with_retry<T: serde::Serialize>(
        &self,
        url: &str,
        event: &T,
        event_id: uuid::Uuid,
        event_type: &str,
    ) -> Result<(), PublishError> {
        const MAX_RETRIES: u32 = 3;
        const INITIAL_BACKOFF_MS: u64 = 100;

        for attempt in 1..=MAX_RETRIES {
            match self.client.post(url).json(event).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!(
                            "✅ Published {} event {} to intel service (attempt {})",
                            event_type, event_id, attempt
                        );
                        return Ok(());
                    } else {
                        let status = response.status();
                        let error_text = response
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unable to read response".to_string());

                        warn!(
                            "⚠️  Intel service returned error for {} event {}: {} - {} (attempt {}/{})",
                            event_type, event_id, status, error_text, attempt, MAX_RETRIES
                        );

                        if attempt < MAX_RETRIES {
                            let backoff_ms = INITIAL_BACKOFF_MS * 4_u64.pow(attempt - 1);
                            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        } else {
                            return Err(PublishError::HttpError {
                                status: status.as_u16(),
                                message: error_text,
                            });
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "⚠️  Failed to publish {} event {} to intel service: {} (attempt {}/{})",
                        event_type, event_id, e, attempt, MAX_RETRIES
                    );

                    if attempt < MAX_RETRIES {
                        let backoff_ms = INITIAL_BACKOFF_MS * 4_u64.pow(attempt - 1);
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    } else {
                        return Err(PublishError::NetworkError(e.to_string()));
                    }
                }
            }
        }

        Err(PublishError::MaxRetriesExceeded)
    }
}

/// Error types for event publishing
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("HTTP error: status {status}, message: {message}")]
    HttpError { status: u16, message: String },

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::types::{CollateralPayload, DealPayload};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_publisher_creation() {
        let publisher = CollateralEventPublisher::new("http://localhost:3010".to_string());
        assert_eq!(publisher.intel_service_url, "http://localhost:3010");
    }

    #[tokio::test]
    async fn test_publish_collateral_event_invalid_url() {
        let publisher = CollateralEventPublisher::new("http://invalid-host-12345".to_string());

        let event = CollateralEvent::CollateralCreated {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            collateral: CollateralPayload {
                id: Uuid::new_v4(),
                name: "Test".to_string(),
                collateral_type: "property".to_string(),
                address: None,
                custom_id: None,
                description: None,
            },
        };

        let result = publisher.publish_collateral(event).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_deal_event_invalid_url() {
        let publisher = CollateralEventPublisher::new("http://invalid-host-12345".to_string());

        let event = DealEvent::DealCreated {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            deal: DealPayload {
                id: Uuid::new_v4(),
                status: "active".to_string(),
                company_id: Uuid::new_v4(),
            },
            collateral_ids: vec![],
        };

        let result = publisher.publish_deal(event).await;
        assert!(result.is_err());
    }
}
