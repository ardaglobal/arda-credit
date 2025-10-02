# Security Setup & Production Deployment Guide

This guide covers security best practices and production deployment for the Arda Credit API with authentication. For monitoring and observability setup, see [`docs/deployment/monitoring.md`](../deployment/monitoring.md).

## Overview

Create a `.env` file in your project root with the following configuration:

```env
# ============================================================================
# DATABASE CONFIGURATION
# ============================================================================
DATABASE_URL=postgresql://username:password@0.0.0.0:5432/arda_credit

# ============================================================================
# SERVER CONFIGURATION  
# ============================================================================
HOST=0.0.0.0
PORT=8080
RUST_LOG=info

# ============================================================================
# JWT AUTHENTICATION
# ============================================================================
# CRITICAL: Change this to a secure random string (minimum 32 characters)
# Generate with: openssl rand -hex 32
JWT_SECRET=your-super-secure-jwt-secret-key-minimum-32-characters-long-change-in-production

# JWT token expiry in hours (default: 24 hours)
JWT_EXPIRY_HOURS=24

# ============================================================================
# CORS CONFIGURATION
# ============================================================================
# Allowed origins for CORS (comma-separated)
CORS_ORIGINS=http://localhost:3000,https://yourdomain.com

# ============================================================================
# RATE LIMITING
# ============================================================================
# Requests per minute per IP/user
RATE_LIMIT_PER_MINUTE=100
```

## Security Checklist

### 1. JWT Security ✅

**What's Already Implemented:**
- ✅ Strong password hashing with Argon2
- ✅ JWT tokens with expiration
- ✅ Secure token validation
- ✅ User context in middleware

**Production Hardening:**
```bash
# Generate a secure JWT secret
openssl rand -hex 32

# Set appropriate token expiry (shorter for higher security)
JWT_EXPIRY_HOURS=1  # 1 hour for high security, 24 hours for convenience
```

### 2. Database Security 🔐

**Migration Security:**
```sql
-- Run the authentication migration
psql $DATABASE_URL -f db/migrations/023_add_authentication_fields.sql

-- Create database indexes for performance
CREATE INDEX CONCURRENTLY idx_identities_login_lookup ON identities(email, password_hash) WHERE password_hash IS NOT NULL;

-- Set up proper database permissions
GRANT SELECT, INSERT, UPDATE ON identities TO api_user;
GRANT SELECT ON identities TO readonly_user;
```

**Connection Security:**
```env
# Use SSL for database connections in production
DATABASE_URL=postgresql://user:pass@host:5432/db?sslmode=require

# Use connection pooling
MAX_DATABASE_CONNECTIONS=20
```

### 3. HTTPS and TLS 🔒

**Reverse Proxy Configuration (Nginx):**
```nginx
server {
    listen 443 ssl http2;
    server_name api.yourdomain.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/private.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES256-GCM-SHA512:DHE-RSA-AES256-GCM-SHA512;
    
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

# Redirect HTTP to HTTPS
server {
    listen 80;
    server_name api.yourdomain.com;
    return 301 https://$server_name$request_uri;
}
```

### 4. Environment-Based Configuration Updates

Update your server initialization to read from environment variables:

```rust
// Add to api/src/server.rs
impl ApiServerConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        use std::env;
        
        let jwt_secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| {
                eprintln!("WARNING: Using default JWT secret. Set JWT_SECRET environment variable in production!");
                "default-secret-change-in-production-at-least-32-characters".to_string()
            });
        
        let jwt_expiry_hours = env::var("JWT_EXPIRY_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse()
            .unwrap_or(24);
        
        let cors_origins = env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "*".to_string())
            .split(',')
            .map(String::from)
            .collect();
        
        let rate_limit = env::var("RATE_LIMIT_PER_MINUTE")
            .unwrap_or_else(|_| "100".to_string())
            .parse()
            .unwrap_or(100);

        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .unwrap_or(8080);

        Ok(Self {
            host,
            port,
            jwt_secret,
            jwt_expiry_hours,
            cors_origins,
            rate_limit_per_minute: rate_limit,
            ..Default::default()
        })
    }
}
```

### 5. Input Validation & Rate Limiting 🛡️

**Update your server startup to use environment config:**
```rust
// In api/src/bin/server.rs
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    // Get configuration from environment
    let config = ApiServerConfig::from_env()?;
    
    // Create server with environment config
    let server = ApiServer::new(None, config).await?;
    
    // Start server...
}
```

### 6. Security Headers Middleware

Add security headers to your server:

```rust
// Add to api/src/server.rs in add_middleware_with_state method
fn add_middleware_with_state(&self, router: Router) -> Router {
    let mut router = router
        .layer(TraceLayer::new_for_http())
        .layer(RequestBodyLimitLayer::new(
            self.config.max_request_size_bytes,
        ))
        .layer(TimeoutLayer::new(Duration::from_secs(
            self.config.request_timeout_seconds,
        )));

    // Add security headers
    router = router.layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    ));
    router = router.layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
        axum::http::header::X_FRAME_OPTIONS,
        axum::http::HeaderValue::from_static("DENY"),
    ));
    router = router.layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
        axum::http::HeaderValue::from_static("x-xss-protection"),
        axum::http::HeaderValue::from_static("1; mode=block"),
    ));

    // Enable compression if configured
    if self.config.enable_compression {
        router = router.layer(CompressionLayer::new());
    }

    // Enable CORS if configured
    if self.config.enable_cors {
        let cors = CorsLayer::new()
            .allow_origin(
                self.config.cors_origins
                    .iter()
                    .map(|origin| origin.parse().unwrap())
                    .collect::<Vec<_>>()
            )
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([
                axum::http::header::AUTHORIZATION,
                axum::http::header::CONTENT_TYPE,
            ]);

        router = router.layer(cors);
    }

    router
}
```

## Production Deployment

### 1. Docker Configuration

Create `Dockerfile`:
```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Build the application
RUN cargo build --release --bin server

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/server /usr/local/bin/server

# Create non-root user
RUN useradd -m -u 1001 appuser
USER appuser

EXPOSE 8080

CMD ["server"]
```

Create `docker-compose.prod.yml`:
```yaml
version: '3.8'

services:
  api:
    build: .
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgresql://arda_user:${DB_PASSWORD}@postgres:5432/arda_credit
      - JWT_SECRET=${JWT_SECRET}
      - JWT_EXPIRY_HOURS=1
      - CORS_ORIGINS=https://yourdomain.com
      - RATE_LIMIT_PER_MINUTE=60
      - RUST_LOG=warn
    depends_on:
      - postgres
    restart: unless-stopped

  postgres:
    image: postgres:15
    environment:
      - POSTGRES_DB=arda_credit
      - POSTGRES_USER=arda_user
      - POSTGRES_PASSWORD=${DB_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./db/migrations:/docker-entrypoint-initdb.d
    restart: unless-stopped

volumes:
  postgres_data:
```

### 2. Kubernetes Deployment

Create `k8s/deployment.yaml`:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: arda-credit-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: arda-credit-api
  template:
    metadata:
      labels:
        app: arda-credit-api
    spec:
      containers:
      - name: api
        image: arda-credit-api:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: arda-secrets
              key: database-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: arda-secrets
              key: jwt-secret
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        readinessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
```

### 3. Monitoring & Alerting

**Health Check Enhancement:**
```rust
// Add to api/src/rest.rs
#[derive(Debug, Serialize, ToSchema)]
pub struct DetailedHealthResponse {
    pub status: String,
    pub timestamp: DateTime<Utc>,
    pub version: String,
    pub database: String,
    pub auth: String,
    pub uptime_seconds: u64,
}

async fn detailed_health_check(State(state): State<ApiState>) -> Json<DetailedHealthResponse> {
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.pool).await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    Json(DetailedHealthResponse {
        status: if db_status == "healthy" { "healthy" } else { "degraded" }.to_string(),
        timestamp: Utc::now(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        database: db_status.to_string(),
        auth: "enabled".to_string(),
        uptime_seconds: 0, // Would implement actual uptime tracking
    })
}
```

### 4. Backup & Recovery

**Database Backup Script:**
```bash
#!/bin/bash
# backup.sh

set -e

DB_NAME="arda_credit"
BACKUP_DIR="/backups"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup
pg_dump $DATABASE_URL > "$BACKUP_DIR/backup_${DATE}.sql"

# Encrypt backup
gpg --cipher-algo AES256 --compress-algo 1 --s2k-mode 3 --s2k-digest-algo SHA512 --s2k-count 65536 --symmetric --output "$BACKUP_DIR/backup_${DATE}.sql.gpg" "$BACKUP_DIR/backup_${DATE}.sql"

# Remove unencrypted backup
rm "$BACKUP_DIR/backup_${DATE}.sql"

# Keep only last 30 days of backups
find $BACKUP_DIR -name "backup_*.sql.gpg" -mtime +30 -delete

echo "Backup completed: backup_${DATE}.sql.gpg"
```

### 5. Security Monitoring

**Log Analysis:**
```bash
# Monitor failed login attempts
grep "Login failed" /var/log/arda-api.log | grep "$(date +%Y-%m-%d)" | wc -l

# Monitor authentication failures
grep "Authentication failed" /var/log/arda-api.log | tail -20

# Monitor rate limiting
grep "Rate limit exceeded" /var/log/arda-api.log | tail -20
```

## Testing Authentication System

### Integration Tests
```rust
#[cfg(test)]
mod auth_integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_auth_flow() {
        // Setup test database
        let pool = setup_test_db().await;
        
        // Register user
        let register_request = RegisterUserRequest {
            email: "test@example.com".to_string(),
            password: "SecureP@ssw0rd123".to_string(),
            jurisdiction: None,
            invite_code: "INVESTOR2025".to_string(),
        };
        
        // Test registration
        let register_response = register_user_endpoint(
            State(test_api_state(&pool)),
            Json(register_request),
        ).await;
        
        assert!(register_response.is_ok());
        
        // Test login
        let login_request = LoginRequest {
            email: "test@example.com".to_string(),
            password: "SecureP@ssw0rd123".to_string(),
        };
        
        let login_response = login_endpoint(
            State(test_api_state(&pool)),
            Json(login_request),
        ).await;
        
        assert!(login_response.is_ok());
        
        // Verify JWT token is valid
        let response = login_response.unwrap().0;
        assert!(!response.access_token.is_empty());
        assert_eq!(response.token_type, "Bearer");
    }
}
```

## Go-Live Checklist

- [ ] JWT secret is cryptographically secure (32+ characters)
- [ ] Database uses SSL connections
- [ ] HTTPS is enabled with valid certificates
- [ ] CORS is configured for specific origins (not *)
- [ ] Rate limiting is appropriately configured
- [ ] Security headers are enabled
- [ ] Database backups are automated and tested
- [ ] Monitoring and alerting are configured
- [ ] Error logging is set up (consider Sentry)
- [ ] Load testing has been performed
- [ ] Security audit has been completed
- [ ] Documentation is updated for operations team

## Troubleshooting

### Common Issues

1. **JWT Token Invalid**
   - Check JWT_SECRET matches between environments
   - Verify token hasn't expired
   - Ensure clock synchronization

2. **Database Connection Issues**
   - Verify DATABASE_URL format
   - Check SSL requirements
   - Confirm user permissions

3. **CORS Errors**
   - Update CORS_ORIGINS environment variable
   - Check protocol (http vs https)
   - Verify Origin headers

4. **Rate Limiting False Positives**
   - Review rate limits for your use case
   - Consider implementing user-specific limits
   - Add IP whitelisting for known good actors

This production setup ensures your authentication system is secure, scalable, and maintainable.
