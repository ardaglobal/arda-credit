# Magic Link Authentication Integration Guide

## Overview

Arda Credit uses **magic link authentication** for secure, passwordless user access. This guide covers the complete authentication flow and API integration.

## Authentication Flow

### 1. Request Magic Link

**Endpoint**: `POST /api/v1/auth/magic-link`

```bash
curl -X POST https://api.arda.credit/api/v1/auth/magic-link \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "redirect_uri": "/dashboard"
  }'
```

**Response**:
```json
{
  "success": true,
  "message": "Magic link sent to your email address",
  "message_id": "ses-message-id-123",
  "expires_in_minutes": 10,
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### 2. Email Content

Users receive an email with a magic link:

```
Subject: Your Arda Credit sign-in link (valid for 10 minutes)

Click this link to sign in to your Arda Credit account:
https://app.arda.xyz/auth/verify?token=ABC123...&state=XYZ789...

This link expires in 10 minutes for your security.
```

### 3. Verify Magic Link

**Endpoint**: `POST /api/v1/auth/verify`

```bash
curl -X POST https://api.arda.credit/api/v1/auth/verify \
  -H "Content-Type: application/json" \
  -d '{
    "token": "ABC123...",
    "state": "XYZ789...",
    "redirect_uri": "/dashboard"
  }'
```

**Response**:
```json
{
  "success": true,
  "message": "Authentication successful",
  "redirect_uri": "/dashboard",
  "user": {
    "user_id": "550e8400-e29b-41d4-a716-446655440000",
    "email": "user@example.com",
    "user_type": "investor",
    "kyc_status": "approved",
    "email_verified": true
  },
  "timestamp": "2024-01-15T10:32:00Z"
}
```

**Session Cookie**: The response includes a secure session cookie:
```
Set-Cookie: session_token=sess_abc123...; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=2592000
```

### 4. Authenticated Requests

Include the session cookie in subsequent requests:

```bash
curl -H "Cookie: session_token=sess_abc123..." \
  https://api.arda.credit/api/v1/auth/me
```

### 5. Get Current User

**Endpoint**: `GET /api/v1/auth/me`

```bash
curl https://api.arda.credit/api/v1/auth/me \
  -H "Cookie: session_token=sess_abc123..."
```

**Response**:
```json
{
  "user_id": "550e8400-e29b-41d4-a716-446655440000",
  "email": "user@example.com",
  "user_type": "investor",
  "kyc_status": "approved",
  "email_verified": true
}
```

### 6. Logout

**Endpoint**: `POST /api/v1/auth/logout`

```bash
curl -X POST https://api.arda.credit/api/v1/auth/logout \
  -H "Cookie: session_token=sess_abc123..."
```

**Response**:
```json
{
  "success": true,
  "message": "Logged out successfully",
  "timestamp": "2024-01-15T11:00:00Z"
}
```

## Error Handling

### Common Error Responses

#### Rate Limit Exceeded (429)
```json
{
  "error": "RATE_LIMIT_EXCEEDED",
  "message": "Too many magic link requests. Please try again later.",
  "details": {
    "limit": 3,
    "window_seconds": 3600,
    "retry_after_seconds": 1200
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### User Not Found (404)
```json
{
  "error": "USER_NOT_FOUND",
  "message": "No account found with this email address",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### Magic Link Expired (410)
```json
{
  "error": "MAGIC_LINK_EXPIRED",
  "message": "This magic link has expired. Please request a new one.",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

#### Invalid State Parameter (400)
```json
{
  "error": "INVALID_STATE",
  "message": "Invalid request. Please start over.",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

## Rate Limiting

### Email Rate Limits
- **3 magic link emails per hour** per email address
- **5 tokens per minute** for API endpoint rate limiting
- **Maximum 3 concurrent active tokens** per user

### Headers
Rate limit information is included in response headers:
```
X-RateLimit-Limit: 3
X-RateLimit-Remaining: 2
X-RateLimit-Window: 3600
```

## Security Features

### Token Security
- **32-byte cryptographically secure tokens**
- **SHA-256 hashing** (plaintext tokens never stored)
- **10-minute expiration** time
- **Single-use tokens** (consumed after verification)

### CSRF Protection
- **State parameter** prevents cross-site request forgery
- **Random 16-byte nonce** generated per request

### Session Security
- **HttpOnly cookies** prevent JavaScript access
- **Secure flag** ensures HTTPS-only transmission
- **SameSite=Lax** prevents CSRF attacks
- **30-day expiration** with automatic cleanup

## Frontend Integration

### JavaScript Example

```javascript
// Request magic link
async function requestMagicLink(email) {
  const response = await fetch('/api/v1/auth/magic-link', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      email: email,
      redirect_uri: '/dashboard'
    }),
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message);
  }

  return await response.json();
}

// Verify magic link (usually from URL parameters)
async function verifyMagicLink(token, state) {
  const response = await fetch('/api/v1/auth/verify', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({
      token: token,
      state: state,
      redirect_uri: '/dashboard'
    }),
    credentials: 'include', // Important: include cookies
  });

  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message);
  }

  return await response.json();
}

// Get current user
async function getCurrentUser() {
  const response = await fetch('/api/v1/auth/me', {
    credentials: 'include', // Important: include cookies
  });

  if (!response.ok) {
    throw new Error('Not authenticated');
  }

  return await response.json();
}

// Logout
async function logout() {
  const response = await fetch('/api/v1/auth/logout', {
    method: 'POST',
    credentials: 'include',
  });

  return await response.json();
}
```

### React Hook Example

```typescript
import { useState, useEffect } from 'react';

interface User {
  user_id: string;
  email: string;
  user_type: string;
  kyc_status: string;
  email_verified: boolean;
}

export function useAuth() {
  const [user, setUser] = useState<User | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    checkAuthStatus();
  }, []);

  const checkAuthStatus = async () => {
    try {
      const response = await fetch('/api/v1/auth/me', {
        credentials: 'include',
      });

      if (response.ok) {
        const userData = await response.json();
        setUser(userData);
      }
    } catch (error) {
      console.error('Auth check failed:', error);
    } finally {
      setLoading(false);
    }
  };

  const requestMagicLink = async (email: string) => {
    const response = await fetch('/api/v1/auth/magic-link', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ email }),
    });

    if (!response.ok) {
      const error = await response.json();
      throw new Error(error.message);
    }

    return await response.json();
  };

  const logout = async () => {
    await fetch('/api/v1/auth/logout', {
      method: 'POST',
      credentials: 'include',
    });
    setUser(null);
  };

  return {
    user,
    loading,
    isAuthenticated: !!user,
    requestMagicLink,
    logout,
    checkAuthStatus,
  };
}
```

## Environment Configuration

See [`MAGIC_LINK_ENV_VARS.md`](../../MAGIC_LINK_ENV_VARS.md) for complete environment variable documentation.

## Testing

### Test Endpoints

For development/testing, you can use the health endpoint:
```bash
curl https://api.arda.credit/health
```

### Postman Collection

A Postman collection with example requests is available at:
`/api-docs/postman-collection.json`

## Migration from Password Authentication

### Key Changes

| Old (Password-based) | New (Magic Link) |
|---------------------|------------------|
| `POST /auth/login` | `POST /auth/magic-link` |
| Password + Email | Email only |
| Bearer tokens | Session cookies |
| JWT validation | Cookie-based sessions |
| Stateless | Session storage |

### Migration Steps

1. **Remove password fields** from your user registration forms
2. **Replace login forms** with email-only magic link request
3. **Update session handling** to use cookies instead of Bearer tokens
4. **Update API calls** to include `credentials: 'include'` for cookies

### Benefits

- **Enhanced Security**: No password storage or transmission
- **Better UX**: No password requirements or resets
- **Reduced Risk**: No password database breaches
- **Simplified Flow**: One-step authentication
