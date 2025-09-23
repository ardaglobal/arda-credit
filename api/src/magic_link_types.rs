use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request to generate a magic link
#[derive(Debug, Deserialize, ToSchema)]
pub struct MagicLinkRequest {
    #[schema(example = "user@example.com")]
    pub email: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "/dashboard")]
    pub redirect_uri: Option<String>,
}

/// Response after requesting a magic link
#[derive(Debug, Serialize, ToSchema)]
pub struct MagicLinkResponse {
    pub success: bool,
    pub message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,

    pub expires_in_minutes: i64,
    pub timestamp: DateTime<Utc>,
}

/// Request to verify a magic link token
#[derive(Debug, Deserialize, ToSchema)]
pub struct VerifyMagicLinkRequest {
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub token: String,

    #[schema(example = "csrf-state-parameter")]
    pub state: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "/dashboard")]
    pub redirect_uri: Option<String>,
}

/// Response after verifying a magic link
#[derive(Debug, Serialize, ToSchema)]
pub struct VerifyMagicLinkResponse {
    pub success: bool,
    pub message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,

    pub user: UserInfo,
    pub timestamp: DateTime<Utc>,
}

/// User information included in verify response
#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub user_id: String,
    pub email: String,
    pub user_type: String,
    pub kyc_status: String,
    pub email_verified: bool,
}

/// Error response for magic link operations
#[derive(Debug, Serialize, ToSchema)]
pub struct MagicLinkErrorResponse {
    pub error: String,
    pub message: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,

    pub timestamp: DateTime<Utc>,
}
