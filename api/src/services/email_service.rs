use aws_config::Region;
use aws_sdk_sesv2::{
    types::{Body, Content, Destination, EmailContent, Message},
    Client as SesClient,
};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

use crate::config::email_config::{EmailConfig, EmailConfigError};
use crate::config::email_templates::{EmailContent as TemplateContent, MagicLinkEmailData};

/// Errors that can occur during email operations
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    #[error("AWS SES error: {0}")]
    SesError(String),
    #[error("Template rendering error: {0}")]
    TemplateError(#[from] askama::Error),
    #[error("Configuration error: {0}")]
    ConfigError(#[from] EmailConfigError),
    #[error("Email service disabled")]
    ServiceDisabled,
    #[error("Invalid email address: {0}")]
    InvalidEmail(String),
}

/// AWS SES-based email service
#[derive(Clone)]
pub struct EmailService {
    client: Arc<SesClient>,
    config: EmailConfig,
}

impl EmailService {
    /// Create a new email service with AWS SES
    #[instrument(level = "info")]
    pub async fn new(config: EmailConfig) -> Result<Self, EmailError> {
        info!(
            "🚀 Initializing AWS SES email service (region: {})",
            config.ses_region
        );

        // Debug log the full configuration (excluding sensitive data)
        debug!(
            "📧 Email service config - enabled: {}, debug_mode: {}, region: {}, from: {}, reply_to: {}, config_set: {:?}",
            config.enabled,
            config.debug_mode,
            config.ses_region,
            config.email_from,
            config.email_reply_to,
            config.configuration_set
        );

        if !config.enabled {
            warn!("📧 Email service is DISABLED via configuration");
            return Ok(Self {
                client: Arc::new(Self::create_mock_client().await),
                config,
            });
        }

        // Create AWS config for the specified region
        debug!("🔧 Creating AWS config for region: {}", config.ses_region);
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::v2024_03_28())
            .region(Region::new(config.ses_region.clone()))
            .load()
            .await;

        debug!("🔧 Creating SES client with AWS config");
        let client = SesClient::new(&aws_config);

        info!("✅ AWS SES client initialized successfully");

        Ok(Self {
            client: Arc::new(client),
            config,
        })
    }

    /// Send a magic link email to a user
    #[instrument(skip(self, magic_link_url), level = "info")]
    pub async fn send_magic_link_email(
        &self,
        user_email: &str,
        magic_link_url: String,
        expiry_minutes: i64,
    ) -> Result<String, EmailError> {
        // Check if in debug mode - log the magic link instead of sending email
        if self.config.debug_mode {
            info!("🔗 DEBUG MODE: Magic link for {}:", user_email);
            info!("      {}", magic_link_url);
            info!("      Expires in {} minutes", expiry_minutes);
            info!(
                "      Click this link to authenticate (expires in {} minutes)",
                expiry_minutes
            );
            return Ok("debug-mode".to_string());
        }

        if !self.config.enabled {
            warn!(
                "📧 Email service disabled - would send magic link to {}",
                user_email
            );
            return Ok("disabled".to_string());
        }

        info!("📧 Sending magic link email to: {}", user_email);

        // Validate email address format (basic check)
        if !user_email.contains('@') || !user_email.contains('.') {
            return Err(EmailError::InvalidEmail(user_email.to_string()));
        }

        // Create email template data
        let template_data =
            MagicLinkEmailData::new(user_email.to_string(), magic_link_url, expiry_minutes);

        // Render email content
        let email_content = TemplateContent::magic_link_email(template_data)?;

        // Send via SES
        let message_id = self.send_email_via_ses(user_email, email_content).await?;

        info!(
            "✅ Magic link email sent successfully (message_id: {})",
            message_id
        );
        Ok(message_id)
    }

    /// Send email via AWS SES
    #[instrument(skip(self, email_content), level = "debug")]
    async fn send_email_via_ses(
        &self,
        recipient: &str,
        email_content: TemplateContent,
    ) -> Result<String, EmailError> {
        debug!(
            "📤 Preparing SES email: from={}, to={}, subject='{}'",
            self.config.email_from, recipient, email_content.subject
        );
        // Create the email message
        let message = Message::builder()
            .subject(
                Content::builder()
                    .data(email_content.subject)
                    .build()
                    .map_err(|e| EmailError::SesError(format!("Subject error: {}", e)))?,
            )
            .body(
                Body::builder()
                    .html(
                        Content::builder()
                            .data(email_content.html_body)
                            .build()
                            .map_err(|e| EmailError::SesError(format!("HTML body error: {}", e)))?,
                    )
                    .text(
                        Content::builder()
                            .data(email_content.text_body)
                            .build()
                            .map_err(|e| EmailError::SesError(format!("Text body error: {}", e)))?,
                    )
                    .build(),
            )
            .build();

        // Create destination
        let destination = Destination::builder().to_addresses(recipient).build();

        // Build the send email request
        let mut request = self
            .client
            .send_email()
            .from_email_address(&self.config.email_from)
            .destination(destination)
            .content(EmailContent::builder().simple(message).build());

        // Add configuration set if configured
        if let Some(config_set) = &self.config.configuration_set {
            request = request.configuration_set_name(config_set);
        }

        // Add reply-to address
        request = request.reply_to_addresses(&self.config.email_reply_to);

        // Log detailed request information before sending
        debug!(
            "📧 SES Request Details: from={}, to={}, reply_to={}, config_set={:?}",
            self.config.email_from,
            recipient,
            self.config.email_reply_to,
            self.config.configuration_set
        );

        // Send the email
        debug!("🚀 Sending email via AWS SES...");
        match request.send().await {
            Ok(output) => {
                let message_id = output.message_id().unwrap_or("unknown").to_string();
                info!("✅ SES send successful: message_id={}", message_id);
                debug!("📧 SES send successful: {}", message_id);
                Ok(message_id)
            }
            Err(e) => {
                error!("❌ SES send failed with detailed error: {:#?}", e);

                // Log additional debugging information
                error!("🔍 SES Error Debug Info:");
                error!("   - Error type: {}", std::any::type_name_of_val(&e));
                error!("   - From address: {}", self.config.email_from);
                error!("   - To address: {}", recipient);
                error!("   - SES region: {}", self.config.ses_region);
                error!(
                    "   - Configuration set: {:?}",
                    self.config.configuration_set
                );

                // Try to extract more specific error information
                let error_message = format!("{}", e);
                if error_message.contains("MessageRejected") {
                    error!("🚫 MessageRejected: Email was rejected by SES (check domain/email verification)");
                } else if error_message.contains("SendingPausedException") {
                    error!("⏸️  SendingPausedException: SES sending is paused for this account");
                } else if error_message.contains("MailFromDomainNotVerifiedException") {
                    error!(
                        "🏷️  MailFromDomainNotVerifiedException: From domain not verified in SES"
                    );
                } else if error_message.contains("ConfigurationSetDoesNotExistException") {
                    error!("⚙️  ConfigurationSetDoesNotExistException: SES configuration set doesn't exist");
                } else if error_message.contains("SignatureDoesNotMatch")
                    || error_message.contains("InvalidSignatureException")
                {
                    error!(
                        "🔑 SignatureDoesNotMatch: AWS credentials issue (check access key/secret)"
                    );
                } else if error_message.contains("AccessDenied")
                    || error_message.contains("UnauthorizedOperation")
                {
                    error!("🛡️  AccessDenied: Insufficient SES permissions for this AWS user");
                } else {
                    error!("❓ Unknown SES error - check AWS SES console for more details");
                }

                Err(EmailError::SesError(format!("SES API error: {}", e)))
            }
        }
    }

    /// Create a mock SES client for when email is disabled
    async fn create_mock_client() -> SesClient {
        let aws_config = aws_config::defaults(aws_config::BehaviorVersion::v2024_03_28())
            .region(Region::new("us-east-1"))
            .load()
            .await;
        SesClient::new(&aws_config)
    }

    /// Test email service connectivity (for health checks)
    #[instrument(skip(self), level = "info")]
    pub async fn test_connectivity(&self) -> Result<bool, EmailError> {
        if !self.config.enabled {
            info!("📧 Email service connectivity: DISABLED (OK)");
            return Ok(true);
        }

        // Test SES connectivity by getting account sending enabled status
        match self.client.get_account().send().await {
            Ok(_) => {
                info!("📧 Email service connectivity: OK");
                Ok(true)
            }
            Err(e) => {
                warn!("📧 Email service connectivity: FAILED - {}", e);
                Err(EmailError::SesError(format!(
                    "Connectivity test failed: {}",
                    e
                )))
            }
        }
    }
}
