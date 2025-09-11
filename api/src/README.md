# Arda Credit API - File Structure and Standards

This document outlines the standardized file structure and coding conventions for the Arda Credit API package.

## 📁 File Structure Overview

```
api/src/
├── lib.rs                           # Main library exports
├── openapi.rs                       # OpenAPI documentation and schemas
├── paths.rs                         # Route constants (single source of truth)
├── server.rs                        # API server setup and launch
├── telemetry.rs                     # Observability and logging
├── types.rs                         # Core API types and configurations
├── *_handlers.rs                    # Handler modules (examples below):
│   ├── admin_handlers.rs            # Admin-only endpoints
│   ├── state_handlers.rs            # State/batch/proof endpoints  
│   └── user_handlers.rs             # User management endpoints
├── *_service.rs                     # Service modules:
│   ├── auth.rs                      # Authentication service
│   ├── balance_service.rs           # Balance verification
│   ├── batch_processor.rs           # Background batch processing
│   ├── kyc_service.rs               # KYC validation service
│   ├── unified_batch_service.rs     # Unified batch operations
│   └── withdrawal_service.rs        # Withdrawal operations
├── middleware.rs                    # Authentication and middleware
├── rest.rs                          # ACTIVE - Remaining API endpoints (7130 lines, reduced from 7323)
├── bin/                             # Binary executables
│   └── server.rs
└── client/                          # Client SDK
    └── mod.rs
```

## 🎯 File Organization Standards

### 1. Single Responsibility Principle
- Each file should have one clear, focused responsibility
- Files should not exceed **1000 lines of code**
- Large files must be split into logical, focused modules

### 2. Handler Files (`*_handlers.rs`)
- Contain only API endpoint handlers and their business logic
- Use OpenAPI decorators for documentation
- Follow consistent error handling patterns
- Import from dedicated service modules for complex operations

### 3. Service Files (`*_service.rs`)
- Contain business logic and data operations
- Should be stateless where possible
- Use dependency injection via constructor parameters
- Handle database operations and external API calls

### 4. Core Infrastructure Files
- `lib.rs` - Main exports and module declarations
- `server.rs` - Server setup, routing, and middleware configuration
- `paths.rs` - All API route constants (single source of truth)
- `openapi.rs` - All OpenAPI schemas and documentation

## 📝 Coding Standards

### Route Definitions
```rust
// ❌ BAD - Hardcoded paths
.route("/api/v1/users", get(list_users))

// ✅ GOOD - Use constants from paths.rs
.route(paths::USERS, get(list_users))
```

### OpenAPI Documentation
```rust
/// Get user details by user ID
#[utoipa::path(
    get,
    path = "/api/v1/user/{user_id}",
    params(("user_id" = String, Path, description = "User ID")),
    responses(
        (status = 200, description = "User details retrieved", body = UserDetailsResponse),
        (status = 404, description = "User not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "User Management"
)]
#[instrument(skip(state), level = "info")]
pub async fn get_user_details_endpoint(
    State(state): State<ApiState>,
    Path(user_id_str): Path<String>,
) -> Result<Json<UserDetailsResponse>, (StatusCode, String)> {
    // Implementation
}
```

### Error Handling
```rust
// ✅ GOOD - Consistent error responses
match operation().await {
    Ok(result) => Ok(Json(result)),
    Err(e) => {
        error!("❌ API: Operation failed: {}", e);
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Operation failed".to_string(),
        ))
    }
}
```

### Logging Standards
```rust
// Use emojis for quick log identification
info!("👤 API: User registered successfully: id={}", user_id);
warn!("⚠️ API: User not found: user_id={}", user_id);
error!("❌ API: Database error: {}", e);
```

## 🔧 Handler File Template

```rust
//! [Module Name] handlers for the Arda Credit API
//!
//! This module contains all [functionality] API endpoints including:
//! - [List of main features]
//!
//! All routes are documented with OpenAPI and use consistent error handling.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};

use crate::openapi::*;
use crate::paths;
use crate::ApiState;
// Import required database functions
// use arda_credit_db::{...};

// Additional schema definitions if needed
// #[derive(Debug, Serialize, Deserialize, ToSchema)]
// pub struct CustomResponse { ... }

// Handler functions...
```

## 🧪 Testing Standards

### Handler Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;

    #[tokio::test]
    async fn test_get_user_details_success() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_get_user_details_not_found() {
        // Test implementation
    }
}
```

### Integration Tests
- Place in `integration.rs`
- Test complete request/response cycles
- Use test database fixtures

## 📚 Documentation Requirements

### OpenAPI Coverage
- All endpoints must have OpenAPI documentation
- All request/response schemas must be defined
- Use consistent status codes and error messages

### Code Documentation
- All public functions must have doc comments
- Complex business logic must be explained
- Database operations should document their purpose

## 🤝 Contribution Guidelines

### Before Adding New Endpoints
1. Check if endpoint fits existing handler files
1. Add route constant to `paths.rs`
1. Add OpenAPI schema to `openapi.rs`
1. Implement handler in appropriate `*_handlers.rs` file
1. Implement services in appropriate `*_service.rs` file
1. Update this README if adding new file types
