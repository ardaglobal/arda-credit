# Arda Credit API Documentation

## Authentication

Arda Credit uses **magic link authentication** for secure, passwordless access.

### Quick Start

1. **Request Magic Link**: `POST /api/v1/auth/magic-link`
2. **Check Email**: Click the magic link in your email
3. **Automatic Login**: Session cookie is set automatically
4. **Make Requests**: Include session cookie in subsequent requests

### Complete Guide

See [Magic Link Authentication Guide](./magic-link-authentication.md) for detailed integration instructions.

### OpenAPI Specification

- **Interactive Docs**: [http://localhost:8080/api-docs](http://localhost:8080/api-docs)
- **JSON Spec**: [http://localhost:8080/api-docs/openapi.json](http://localhost:8080/api-docs/openapi.json)

### Authentication Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/v1/auth/magic-link` | POST | Request magic link via email |
| `/api/v1/auth/verify` | POST | Verify magic link and create session |
| `/api/v1/auth/me` | GET | Get current authenticated user |
| `/api/v1/auth/logout` | POST | Logout and invalidate session |

~~**Deprecated**: Password-based authentication has been removed.~~

## Session Management

All protected endpoints require a valid session cookie. Sessions are automatically managed through magic link authentication.

### Protected Endpoints

Most API endpoints require authentication. Anonymous endpoints include:

- `GET /health`
- `GET /api/v1/info`
- `POST /api/v1/user` (registration)
- Magic link authentication endpoints

For all other endpoints, include the session cookie received after magic link verification.

## OpenAPI Documentation

The API is fully documented using OpenAPI 3.0 specifications with interactive Swagger UI.

### Interactive Documentation

- **Swagger UI**: [http://localhost:8080/api-docs](http://localhost:8080/api-docs)
  - Try endpoints directly in your browser
  - View request/response schemas
  - Test authentication flows

- **OpenAPI JSON**: [http://localhost:8080/api-docs/openapi.json](http://localhost:8080/api-docs/openapi.json)
  - Import into Postman, Insomnia, or other API tools
  - Generate client SDKs for your preferred language

### API Endpoints Overview

| Category | Endpoints | Description |
|----------|-----------|-------------|
| **Authentication** | `/api/v1/auth/*` | Magic link authentication |
| **Users** | `/api/v1/user/*` | User registration and profiles |
| **Loans** | `/api/v1/loan/*` | Loan origination and management |
| **Payments** | `/api/v1/payment/*` | Payment processing |
| **Pools** | `/api/v1/pool/*` | Liquidity pool management |
| **Withdrawals** | `/api/v1/withdrawal/*` | USDC withdrawal operations |
| **State** | `/api/v1/state/*` | Batch processing and ZK proofs |
| **Admin** | `/api/v1/admin/*` | Administrative operations |

### Schema Documentation

All request and response models are fully documented with:

- Field descriptions and examples
- Required/optional field indicators
- Data type validation rules
- Enum values where applicable

### Code Generation

The OpenAPI specification supports automatic code generation for:

- **Frontend**: TypeScript interfaces, React hooks
- **Backend**: API clients in Python, Java, Go
- **Mobile**: iOS/Android SDKs
- **Testing**: Automated test generation

## Development Workflow

### Testing API Changes

1. **Start the server**: `make server`
2. **Open Swagger UI**: Visit `http://localhost:8080/api-docs`
3. **Test endpoints**: Use the "Try it out" buttons
4. **Check schemas**: View request/response models

### Authentication in Swagger UI

To test protected endpoints in Swagger UI:

1. **Request magic link** using the auth endpoint
2. **Check email** and click the magic link
3. **Session cookie** will be set automatically
4. **All subsequent requests** will include the session cookie

## Common Integration Patterns

### JavaScript/TypeScript

```javascript
// Authenticated request with session cookie
const response = await fetch('/api/v1/auth/me', {
  credentials: 'include'  // Important: includes session cookie
});
```

### Python

```python
import requests

# Include session cookie from previous auth
cookies = {'session_token': 'your_session_token'}
response = requests.get('/api/v1/auth/me', cookies=cookies)
```

### cURL

```bash
# Include session cookie
curl -H "Cookie: session_token=your_session_token" \
  http://localhost:8080/api/v1/auth/me
```

## Troubleshooting

### Common Issues

1. **401 Unauthorized**: Session cookie missing or expired
2. **403 Forbidden**: User doesn't have permission for the operation
3. **429 Too Many Requests**: Rate limit exceeded
4. **422 Unprocessable Entity**: Invalid request data

### Debug Headers

Enable debug headers by setting the environment variable:
```bash
DEBUG_HEADERS=true
```

This adds detailed information about authentication and rate limiting to response headers.

## Version History

- **v1.0**: Initial magic link authentication implementation
- **v1.1**: Added liquidity pool management
- **v1.2**: Enhanced withdrawal system
- **v1.3**: Batch processing improvements