# Magic Link Authentication Environment Variables

Add these environment variables to your `.env` file for the magic link authentication system:

## Required Environment Variables

```bash
# Magic Link Configuration
MAGIC_BASE_URL=https://app.arda.xyz/auth/verify
MAGIC_LINK_TTL_MINUTES=10
MAGIC_LINK_MAX_TOKENS_PER_USER=3

# AWS SES Configuration (for email sending)
SES_REGION=us-east-1
EMAIL_FROM=auth@arda.xyz
EMAIL_REPLY_TO=support@arda.xyz
SES_CONFIGURATION_SET=tx-auth

# Session Configuration
SESSION_EXPIRY_DAYS=30
SESSION_COOKIE_DOMAIN=.arda.xyz
```

## Variable Descriptions

### Magic Link Settings
- `MAGIC_BASE_URL`: The base URL for magic link verification (where users are redirected)
- `MAGIC_LINK_TTL_MINUTES`: How long magic links are valid (default: 10 minutes)
- `MAGIC_LINK_MAX_TOKENS_PER_USER`: Maximum concurrent active magic links per user (default: 3)

### AWS SES Settings
- `SES_REGION`: AWS region where SES is configured
- `EMAIL_FROM`: From address for magic link emails
- `EMAIL_REPLY_TO`: Reply-to address for support
- `SES_CONFIGURATION_SET`: SES configuration set for email tracking

### Session Settings
- `SESSION_EXPIRY_DAYS`: How long user sessions remain valid (default: 30 days)
- `SESSION_COOKIE_DOMAIN`: Domain for session cookies (should match your app domain)

## Development vs Production

For development:
```bash
MAGIC_BASE_URL=http://localhost:8080/auth/verify
SESSION_COOKIE_DOMAIN=localhost
```

For production:
```bash
MAGIC_BASE_URL=https://app.arda.xyz/auth/verify
SESSION_COOKIE_DOMAIN=.arda.xyz
```
