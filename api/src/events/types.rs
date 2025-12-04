//! Event types for collateral intelligence service integration
//!
//! These types match the arda-collateral-intel OpenAPI specification exactly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// COLLATERAL EVENTS
// ============================================================================

/// Collateral events from arda-credit
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum CollateralEvent {
    /// Collateral was created
    CollateralCreated {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        collateral: CollateralPayload,
    },
    /// Collateral was updated
    CollateralUpdated {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        collateral: CollateralPayload,
        changes: Vec<FieldChange>,
    },
    /// Collateral was deleted
    CollateralDeleted {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        collateral_id: Uuid,
    },
}

/// Collateral payload from arda-credit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollateralPayload {
    pub id: Uuid,
    pub name: String,
    pub collateral_type: String,
    pub company_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

// ============================================================================
// DEAL EVENTS
// ============================================================================

/// Deal events from arda-credit
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DealEvent {
    /// Deal was created
    DealCreated {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal: DealPayload,
        collateral_ids: Vec<Uuid>,
    },
    /// Deal status changed
    DealStatusChanged {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal_id: Uuid,
        previous_status: String,
        new_status: String,
        collateral_ids: Vec<Uuid>,
    },
    /// Collateral was linked to a deal
    DealCollateralLinked {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal_id: Uuid,
        collateral_ids: Vec<Uuid>,
    },
    /// Collateral was unlinked from a deal
    DealCollateralUnlinked {
        event_id: Uuid,
        timestamp: DateTime<Utc>,
        deal_id: Uuid,
        collateral_id: Uuid,
    },
}

/// Deal payload from arda-credit events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DealPayload {
    pub id: Uuid,
    pub status: String,
    pub company_id: Uuid,
}

// ============================================================================
// SHARED TYPES
// ============================================================================

/// Field change tracking for collateral updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldChange {
    pub field_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

impl CollateralEvent {
    /// Get the event ID
    pub fn event_id(&self) -> Uuid {
        match self {
            Self::CollateralCreated { event_id, .. }
            | Self::CollateralUpdated { event_id, .. }
            | Self::CollateralDeleted { event_id, .. } => *event_id,
        }
    }

    /// Get the event timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::CollateralCreated { timestamp, .. }
            | Self::CollateralUpdated { timestamp, .. }
            | Self::CollateralDeleted { timestamp, .. } => *timestamp,
        }
    }
}

impl DealEvent {
    /// Get the event ID
    pub fn event_id(&self) -> Uuid {
        match self {
            Self::DealCreated { event_id, .. }
            | Self::DealStatusChanged { event_id, .. }
            | Self::DealCollateralLinked { event_id, .. }
            | Self::DealCollateralUnlinked { event_id, .. } => *event_id,
        }
    }

    /// Get the event timestamp
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::DealCreated { timestamp, .. }
            | Self::DealStatusChanged { timestamp, .. }
            | Self::DealCollateralLinked { timestamp, .. }
            | Self::DealCollateralUnlinked { timestamp, .. } => *timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collateral_event_serialization() {
        let event = CollateralEvent::CollateralCreated {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            collateral: CollateralPayload {
                id: Uuid::new_v4(),
                name: "Test Collateral".to_string(),
                collateral_type: "property".to_string(),
                company_id: Uuid::new_v4(),
                address: Some("123 Main St".to_string()),
                custom_id: None,
                description: None,
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("CollateralCreated"));
        assert!(json.contains("event_type"));

        let deserialized: CollateralEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_id(), event.event_id());
    }

    #[test]
    fn test_deal_event_serialization() {
        let event = DealEvent::DealCreated {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            deal: DealPayload {
                id: Uuid::new_v4(),
                status: "active".to_string(),
                company_id: Uuid::new_v4(),
            },
            collateral_ids: vec![Uuid::new_v4()],
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("DealCreated"));
        assert!(json.contains("event_type"));

        let deserialized: DealEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.event_id(), event.event_id());
    }
}
