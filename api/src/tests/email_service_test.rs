#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::email_config::EmailConfig;
    use crate::services::email_service::EmailService;

    #[tokio::test]
    async fn test_email_service_disabled() {
        let config = EmailConfig {
            ses_region: "us-east-1".to_string(),
            email_from: "test@example.com".to_string(),
            email_reply_to: "support@example.com".to_string(),
            configuration_set: None,
            magic_base_url: "https://example.com/verify".to_string(),
            enabled: false, // Disabled for test
            debug_mode: false,
        };

        let email_service = EmailService::new(config).await.unwrap();

        let result = email_service.send_magic_link_email(
            "test@example.com",
            "https://example.com/verify?token=test".to_string(),
            10,
        ).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "disabled");
    }

    #[tokio::test]
    async fn test_email_template_rendering() {
        use crate::config::email_templates::{EmailContent, MagicLinkEmailData};

        let data = MagicLinkEmailData::new(
            "test@example.com".to_string(),
            "https://example.com/verify?token=test".to_string(),
            10,
        );

        let content = EmailContent::magic_link_email(data).unwrap();

        assert!(content.subject.contains("Arda Credit"));
        assert!(content.html_body.contains("test@example.com"));
        assert!(content.text_body.contains("https://example.com/verify"));
    }

    #[tokio::test]
    async fn test_email_service_connectivity() {
        // Test with disabled service
        let config = EmailConfig {
            ses_region: "us-east-1".to_string(),
            email_from: "test@example.com".to_string(),
            email_reply_to: "support@example.com".to_string(),
            configuration_set: None,
            magic_base_url: "https://example.com/verify".to_string(),
            enabled: false,
            debug_mode: false,
        };

        let email_service = EmailService::new(config).await.unwrap();
        let connectivity = email_service.test_connectivity().await;

    assert!(connectivity.is_ok());
    assert!(connectivity.unwrap());
}

#[tokio::test]
async fn test_email_service_debug_mode() {
    let config = EmailConfig {
        ses_region: "us-east-1".to_string(),
        email_from: "test@example.com".to_string(),
        email_reply_to: "support@example.com".to_string(),
        configuration_set: None,
        magic_base_url: "https://example.com/verify".to_string(),
        enabled: true, // Enable but set debug mode
        debug_mode: true,
    };

    let email_service = EmailService::new(config).await.unwrap();

    let result = email_service.send_magic_link_email(
        "test@example.com",
        "https://example.com/verify?token=test&state=test".to_string(),
        10,
    ).await;

    // Should succeed and return debug message ID
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "debug-mode");
}
