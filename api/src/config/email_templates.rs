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

/// Team invitation email template data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamInvitationEmailData {
    pub invitee_email: String,
    pub company_name: String,
    pub inviter_name: String,
    pub role: String,         // "admin" or "user"
    pub profile_type: String, // "investor" or "originator"
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

/// HTML email template for team invitations
#[derive(Template)]
#[template(path = "team_invitation_email.html")]
pub struct TeamInvitationHtmlTemplate {
    pub data: TeamInvitationEmailData,
}

/// Plain text email template for team invitations
#[derive(Template)]
#[template(path = "team_invitation_email.txt")]
pub struct TeamInvitationTextTemplate {
    pub data: TeamInvitationEmailData,
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

impl TeamInvitationEmailData {
    pub fn new(
        invitee_email: String,
        company_name: String,
        inviter_name: String,
        role: String,
        profile_type: String,
        magic_link_url: String,
        expiry_minutes: i64,
    ) -> Self {
        Self {
            invitee_email,
            company_name,
            inviter_name,
            role,
            profile_type,
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

    pub fn team_invitation_email(data: TeamInvitationEmailData) -> Result<Self, askama::Error> {
        let html_template = TeamInvitationHtmlTemplate { data: data.clone() };
        let text_template = TeamInvitationTextTemplate { data: data.clone() };

        Ok(Self {
            subject: format!(
                "You've been invited to join {} on {}",
                data.company_name, data.app_name
            ),
            html_body: html_template.render()?,
            text_body: text_template.render()?,
        })
    }
}
