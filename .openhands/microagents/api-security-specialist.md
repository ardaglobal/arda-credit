---
name: API Security and Authentication Specialist
type: knowledge
version: 1.0.0
agent: CodeActAgent
triggers: ["api", "security", "authentication", "jwt", "argon2", "validation", "middleware", "axum"]
---

# API Security and Authentication Specialist

This microagent specializes in API security, authentication, authorization, and secure web service operations within the Arda Credit ecosystem.

## Domain Expertise

### API Security Components
- **Authentication**: `api/src/auth.rs` - JWT + Argon2 authentication system
- **Middleware**: `api/src/middleware.rs` - Security middleware and request validation
- **User Handlers**: `api/src/user_handlers.rs` - User registration and management
- **Auth Handlers**: `api/src/authentication_handlers.rs` - Login/logout endpoints

### API Architecture Scale
- **Axum Framework**: 1,323+ async/tokio/axum references across codebase
- **Handler Files**: 15+ specialized handler files for different domains
- **Security Layers**: Multi-layer security with authentication, authorization, and validation
- **Input Validation**: Comprehensive validation using `validator` crate

### Current Implementation Analysis
- **Authentication**: Argon2 password hashing + JWT token management
- **Authorization**: Role-based access control (investor, lender, merchant)
- **Input Validation**: Email validation, password complexity requirements
- **Error Handling**: Structured error responses with proper HTTP status codes

## Specialized Responsibilities

### 1. Authentication Security
- **Password Security**: Argon2 parameter tuning and security validation
- **JWT Management**: Token generation, validation, and revocation strategies
- **Session Security**: Secure session management and token lifecycle
- **Multi-Factor Authentication**: Implementation and security considerations

### 2. API Endpoint Security
- **Input Validation**: Comprehensive validation of all API inputs
- **Rate Limiting**: Protection against abuse and DoS attacks
- **CORS Configuration**: Proper cross-origin resource sharing setup
- **Request Sanitization**: Prevention of injection attacks

### 3. Authorization and Access Control
- **Role-Based Access**: Proper enforcement of user roles and permissions
- **Resource Authorization**: Ensure users can only access their own resources
- **Admin Operations**: Secure admin endpoints with proper authorization
- **API Key Management**: Secure API key generation and validation

## Code Quality Standards

### Secure Authentication
```rust
// Good: Proper password hashing with Argon2
pub async fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AuthError::HashingError)?
        .to_string();
    
    Ok(password_hash)
}

// Avoid: Weak password hashing
fn hash_password_weak(password: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize()) // No salt, weak algorithm
}
```

### Input Validation
```rust
// Good: Comprehensive input validation
#[derive(Debug, Validate, Deserialize)]
pub struct CreateUserRequest {
    #[validate(email(message = "Invalid email format"))]
    #[validate(length(max = 255, message = "Email too long"))]
    pub email: String,
    
    #[validate(length(min = 8, max = 128, message = "Password must be 8-128 characters"))]
    #[validate(custom(function = "validate_password_complexity"))]
    pub password: String,
    
    #[validate(length(min = 1, max = 100, message = "Name required"))]
    pub name: String,
}

// Avoid: No input validation
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub email: String,    // No validation
    pub password: String, // No complexity requirements
    pub name: String,     // No length limits
}
```

### Secure Error Handling
```rust
// Good: Secure error responses
pub enum ApiError {
    Unauthorized,
    Forbidden,
    ValidationError(String),
    InternalError, // No details exposed
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, &msg),
            ApiError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        
        (status, Json(json!({"error": message}))).into_response()
    }
}

// Avoid: Exposing internal details
fn handle_error(e: DatabaseError) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)).into_response()
}
```

## Security Audit Checklist

### Authentication Security
- [ ] Argon2 parameters are properly configured for security
- [ ] JWT tokens have appropriate expiration times
- [ ] Password complexity requirements are enforced
- [ ] Account lockout mechanisms prevent brute force attacks
- [ ] Secure password reset functionality

### Authorization Security
- [ ] All endpoints require proper authentication
- [ ] Role-based access control is properly enforced
- [ ] Users can only access their own resources
- [ ] Admin endpoints have additional security measures
- [ ] API keys are properly validated and scoped

### Input Security
- [ ] All inputs are validated using the `validator` crate
- [ ] SQL injection prevention through parameterized queries
- [ ] XSS prevention through proper output encoding
- [ ] File upload security (if applicable)
- [ ] Request size limits to prevent DoS

### Transport Security
- [ ] HTTPS enforced for all API endpoints
- [ ] Proper TLS configuration and certificate management
- [ ] HSTS headers configured
- [ ] Secure cookie settings
- [ ] CORS properly configured

## Integration Points

### With Financial Layer
- **Transaction Authorization**: Verify user permissions for financial operations
- **Balance Access**: Ensure users can only access their own balance information
- **Audit Logging**: Log all financial API operations for compliance

### With Database Layer
- **Secure Queries**: All database queries use parameterized statements
- **Connection Security**: Secure database connection management
- **Data Access Control**: Proper authorization for data access

### With Zero-Knowledge Layer
- **Proof Authorization**: Verify user permissions for proof operations
- **Privacy Protection**: Ensure API doesn't leak private information
- **Batch Access Control**: Proper authorization for batch operations

## Performance Optimization

### Authentication Performance
- **JWT Caching**: Cache JWT validation results appropriately
- **Password Hashing**: Optimize Argon2 parameters for performance vs security
- **Session Management**: Efficient session storage and retrieval
- **Rate Limiting**: Efficient rate limiting implementation

### API Performance
- **Request Processing**: Optimize request parsing and validation
- **Response Caching**: Cache appropriate API responses
- **Connection Pooling**: Efficient database connection management
- **Async Operations**: Proper async/await usage for non-blocking operations

## Monitoring and Observability

### Security Metrics
- Authentication success/failure rates
- Failed login attempts and patterns
- API endpoint access patterns
- Rate limiting trigger frequency

### Performance Metrics
- API response times by endpoint
- Authentication operation latency
- Request validation times
- Error rates by endpoint and error type

### Compliance Metrics
- Audit log completeness
- Access control enforcement
- Data access patterns
- Security incident detection

## Handler File Organization

### Current Large Files Requiring Refactoring
1. **`api/src/financial_transaction_handlers.rs` (1,599 lines)**:
   - Split by transaction type (deposits, withdrawals, transfers)
   - Extract validation logic to separate modules
   - Separate handler logic from business logic

2. **`api/src/pool_handlers.rs` (1,336 lines)**:
   - Split by pool operation type
   - Extract pool validation logic
   - Separate admin vs user operations

3. **`api/src/credit_line_handlers.rs` (1,184 lines)**:
   - Split by credit line operation
   - Extract credit assessment logic
   - Separate creation from management operations

## Best Practices

### Development
- Always validate all inputs using the `validator` crate
- Use proper error handling that doesn't expose internal details
- Implement comprehensive logging for security events
- Follow principle of least privilege for all operations

### Testing
- Test all authentication and authorization scenarios
- Verify input validation with edge cases and malicious inputs
- Test rate limiting and DoS protection mechanisms
- Validate error handling doesn't leak sensitive information

### Deployment
- Use HTTPS for all API endpoints in production
- Implement proper monitoring and alerting for security events
- Regular security audits and penetration testing
- Keep all dependencies updated for security patches

### Security
- Regular review of authentication and authorization logic
- Monitor for suspicious API usage patterns
- Implement proper incident response procedures
- Regular security training for development team

This specialist microagent ensures the API security and authentication components of Arda Credit maintain the highest standards of security while providing efficient and user-friendly access to the system's capabilities.