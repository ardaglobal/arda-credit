use std::env;

/// Configuration for AWS SES email service
#[derive(Debug, Clone)]
pub struct EmailConfig {
    pub ses_region: String,
    pub email_from: String,
    pub email_reply_to: String,
    pub configuration_set: Option<String>,
    pub magic_base_url: String,
    pub enabled: bool,
}

impl EmailConfig {
    pub fn from_env() -> Result<Self, EmailConfigError> {
        Ok(Self {
            ses_region: env::var("SES_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            email_from: env::var("EMAIL_FROM")
                .map_err(|_| EmailConfigError::MissingRequired("EMAIL_FROM".to_string()))?,
            email_reply_to: env::var("EMAIL_REPLY_TO")
                .unwrap_or_else(|_| "support@arda.xyz".to_string()),
            configuration_set: env::var("SES_CONFIGURATION_SET").ok(),
            magic_base_url: env::var("MAGIC_BASE_URL")
                .unwrap_or_else(|_| "https://app.arda.xyz/auth/verify".to_string()),
            enabled: env::var("EMAIL_ENABLED")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmailConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingRequired(String),
    #[error("Invalid configuration value: {0}")]
    InvalidValue(String),
}
