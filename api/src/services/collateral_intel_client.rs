//! Client for arda-collateral-intel service integration
//!
//! Provides HTTP client to call the arda-collateral-intel service endpoints,
//! specifically for fetching detection history to merge with audit trail.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use uuid::Uuid;

/// Client for communicating with arda-collateral-intel service
#[derive(Clone)]
pub struct CollateralIntelClient {
    base_url: String,
    client: reqwest::Client,
}

impl CollateralIntelClient {
    /// Create a new CollateralIntelClient
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Get detection history for a collateral asset
    ///
    /// Returns None if the service is unavailable or the collateral has no detection history.
    pub async fn get_detection_history(
        &self,
        collateral_id: Uuid,
    ) -> Result<Option<DetectionHistoryResponse>, ClientError> {
        let url = format!(
            "{}/api/v1/collateral/{}/detection-history",
            self.base_url, collateral_id
        );

        debug!(
            "Fetching detection history from arda-collateral-intel: {}",
            url
        );

        let response = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| {
                warn!("Failed to connect to arda-collateral-intel: {}", e);
                ClientError::NetworkError(e.to_string())
            })?;

        match response.status() {
            reqwest::StatusCode::OK => {
                let history: DetectionHistoryResponse = response.json().await.map_err(|e| {
                    warn!("Failed to parse detection history response: {}", e);
                    ClientError::ParseError(e.to_string())
                })?;
                Ok(Some(history))
            }
            reqwest::StatusCode::NOT_FOUND => {
                debug!(
                    "No detection history found for collateral {}",
                    collateral_id
                );
                Ok(None)
            }
            reqwest::StatusCode::SERVICE_UNAVAILABLE => {
                warn!("arda-collateral-intel service is unavailable");
                Ok(None)
            }
            status => {
                let body = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unable to read response body".to_string());
                warn!("arda-collateral-intel returned error {}: {}", status, body);
                Err(ClientError::ServiceError {
                    status: status.as_u16(),
                    message: body,
                })
            }
        }
    }
}

// ============================================================================
// RESPONSE TYPES (matching arda-collateral-intel API)
// ============================================================================

/// Detection history response from arda-collateral-intel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionHistoryResponse {
    pub collateral_id: String,
    pub total_count: i32,
    pub limit: i32,
    pub offset: i32,
    pub detections: Vec<DetectionRecord>,
}

/// Individual detection record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionRecord {
    pub detection_id: String,
    pub trigger_type: String,
    pub status: String,
    pub match_count: usize,
    pub analyzed_at: String, // ISO 8601 timestamp
    pub matches: Vec<DetectionMatchDetail>,
}

/// Detection match details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionMatchDetail {
    pub matched_collateral_id: String,
    pub matched_deal_id: Option<String>,
    pub overall_score: f32,
    pub confidence: String,
    pub is_same_deal: bool,
    pub both_deals_active: bool,
    pub summary: String,
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Service error (status {status}): {message}")]
    ServiceError { status: u16, message: String },
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Try to parse a detection timestamp to DateTime<Utc>
pub fn parse_detection_timestamp(timestamp_str: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(timestamp_str)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_detection_timestamp() {
        let timestamp = "2025-12-10T10:30:00Z";
        let parsed = parse_detection_timestamp(timestamp);
        assert!(parsed.is_some());
    }

    #[test]
    fn test_parse_invalid_timestamp() {
        let timestamp = "not-a-timestamp";
        let parsed = parse_detection_timestamp(timestamp);
        assert!(parsed.is_none());
    }
}
