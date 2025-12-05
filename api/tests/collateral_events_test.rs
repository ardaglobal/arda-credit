//! Integration tests for collateral event publishing

#[cfg(test)]
mod tests {
    use api::events::{CollateralEvent, CollateralPayload, DealEvent, DealPayload, FieldChange};
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_collateral_event_serialization() {
        let event = CollateralEvent::CollateralCreated {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            collateral: CollateralPayload {
                id: Uuid::new_v4(),
                name: "Test Property".to_string(),
                collateral_type: "real_estate".to_string(),
                address: Some("123 Main St".to_string()),
                custom_id: Some("PROP-001".to_string()),
                description: Some("Office building".to_string()),
            },
        };

        // Test serialization
        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("CollateralCreated"));
        assert!(json.contains("event_type"));
        assert!(json.contains("Test Property"));

        // Test deserialization
        let deserialized: CollateralEvent =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.event_id(), event.event_id());
    }

    #[test]
    fn test_collateral_updated_event() {
        let event = CollateralEvent::CollateralUpdated {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            collateral: CollateralPayload {
                id: Uuid::new_v4(),
                name: "Updated Property".to_string(),
                collateral_type: "commercial".to_string(),
                address: None,
                custom_id: None,
                description: None,
            },
            changes: vec![FieldChange {
                field_name: "name".to_string(),
                old_value: Some("Old Name".to_string()),
                new_value: Some("Updated Property".to_string()),
            }],
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("CollateralUpdated"));
        assert!(json.contains("changes"));
    }

    #[test]
    fn test_collateral_deleted_event() {
        let collateral_id = Uuid::new_v4();
        let event = CollateralEvent::CollateralDeleted {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            collateral_id,
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("CollateralDeleted"));
        assert!(json.contains(&collateral_id.to_string()));
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
            collateral_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("DealCreated"));
        assert!(json.contains("collateral_ids"));
        assert!(json.contains("active"));

        let deserialized: DealEvent = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.event_id(), event.event_id());
    }

    #[test]
    fn test_deal_status_changed_event() {
        let event = DealEvent::DealStatusChanged {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            deal_id: Uuid::new_v4(),
            previous_status: "originated".to_string(),
            new_status: "active".to_string(),
            collateral_ids: vec![Uuid::new_v4()],
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("DealStatusChanged"));
        assert!(json.contains("previous_status"));
        assert!(json.contains("new_status"));
        assert!(json.contains("originated"));
        assert!(json.contains("active"));
    }

    #[test]
    fn test_deal_collateral_linked_event() {
        let event = DealEvent::DealCollateralLinked {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            deal_id: Uuid::new_v4(),
            collateral_ids: vec![Uuid::new_v4()],
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("DealCollateralLinked"));
    }

    #[test]
    fn test_deal_collateral_unlinked_event() {
        let event = DealEvent::DealCollateralUnlinked {
            event_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            deal_id: Uuid::new_v4(),
            collateral_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&event).expect("Failed to serialize");
        assert!(json.contains("DealCollateralUnlinked"));
    }

    #[test]
    fn test_event_type_discriminator() {
        // Test that event_type field is correctly used as discriminator
        let json = r#"{
            "event_type": "CollateralCreated",
            "event_id": "550e8400-e29b-41d4-a716-446655440000",
            "timestamp": "2025-12-04T10:00:00Z",
            "collateral": {
                "id": "650e8400-e29b-41d4-a716-446655440000",
                "name": "Test",
                "collateral_type": "property"
            }
        }"#;

        let event: CollateralEvent = serde_json::from_str(json).expect("Failed to deserialize");
        match event {
            CollateralEvent::CollateralCreated { .. } => {}
            _ => panic!("Expected CollateralCreated variant"),
        }
    }
}
