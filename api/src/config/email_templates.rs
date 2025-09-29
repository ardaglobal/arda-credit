use askama::Template;
use serde::{Deserialize, Serialize};

/// Magic link email template data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MagicLinkEmailData {
    pub user_email: String,
    pub magic_link_url: String,
    pub expiry_minutes: i64,
    pub app_name: String,
    pub support_email: String,
}

/// HTML email template for magic links
#[derive(Template)]
#[template(path = "magic_link_email.html")]
pub struct MagicLinkHtmlTemplate {
    pub data: MagicLinkEmailData,
}

/// Plain text email template for magic links
#[derive(Template)]
#[template(path = "magic_link_email.txt")]
pub struct MagicLinkTextTemplate {
    pub data: MagicLinkEmailData,
}

impl MagicLinkEmailData {
    pub fn new(user_email: String, magic_link_url: String, expiry_minutes: i64) -> Self {
        Self {
            user_email,
            magic_link_url,
            expiry_minutes,
            app_name: "Arda Credit".to_string(),
            support_email: "support@arda.xyz".to_string(),
        }
    }
}

/// Email content with both HTML and text versions
#[derive(Debug, Clone)]
pub struct EmailContent {
    pub subject: String,
    pub html_body: String,
    pub text_body: String,
}

impl EmailContent {
    pub fn magic_link_email(data: MagicLinkEmailData) -> Result<Self, askama::Error> {
        let html_template = MagicLinkHtmlTemplate { data: data.clone() };
        let text_template = MagicLinkTextTemplate { data: data.clone() };

        Ok(Self {
            subject: format!(
                "Your {} sign-in link (valid for {} minutes)",
                data.app_name, data.expiry_minutes
            ),
            html_body: html_template.render()?,
            text_body: text_template.render()?,
        })
    }
}
