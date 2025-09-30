use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use time::OffsetDateTime;

/// Create a secure session cookie
pub fn create_session_cookie(
    token: String,
    expires_in_days: i64,
    domain: Option<String>,
) -> Cookie<'static> {
    let expires = Utc::now() + Duration::days(expires_in_days);
    let expires_time = OffsetDateTime::from_unix_timestamp(expires.timestamp()).unwrap();

    let mut cookie = Cookie::build(("session_token", token))
        .expires(expires_time)
        .http_only(true)
        .secure(true) // Only over HTTPS in production
        .same_site(SameSite::Lax)
        .path("/");

    if let Some(domain) = domain {
        cookie = cookie.domain(domain);
    }

    cookie.build()
}

/// Create a cookie that expires the session (for logout)
pub fn create_expired_session_cookie() -> Cookie<'static> {
    Cookie::build(("session_token", ""))
        .expires(OffsetDateTime::from_unix_timestamp(0).unwrap())
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .path("/")
        .build()
}
