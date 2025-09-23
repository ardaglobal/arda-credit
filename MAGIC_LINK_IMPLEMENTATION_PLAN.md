# Magic Link Authentication Implementation Plan

## Overview

This document outlines the plan to replace Arda Credit's password-based authentication with magic link authentication. This is a **complete cutover** for the beta/proof-of-concept environment - no backwards compatibility needed.

## Current State Analysis

### Existing Authentication System
- **Password-based JWT authentication** with Argon2 password hashing
- **Stateless JWT tokens** 
- **PostgreSQL database** with `identities` table containing password fields
- **In-memory rate limiting** using token bucket algorithm
- **Basic email verification tokens** (not currently used for auth)

### Changes Required
- ✅ **Add AWS SES integration** - email sending capability
- ✅ **Add session management** - replace stateless JWT with sessions
- ✅ **Add magic link endpoints** - new API routes
- ✅ **Add magic link database schema** - new tables for tokens and sessions
- ✅ **Remove password logic** - complete cleanup of password-based auth

## Target Architecture

### New Endpoints
- `POST /auth/magic-link` - Request magic link via email
- `POST /auth/verify` - Verify magic link token and create session
- `GET /auth/me` - Get current authenticated user (session-based)
- `POST /auth/logout` - Invalidate session

### Authentication Flow
1. User enters email → `POST /auth/magic-link`
2. System generates token, stores hash, sends email
3. User clicks link → calls `POST /auth/verify` → creates server session
4. Session-based authentication for subsequent requests

## Phased Implementation Approach (Updated)

### Critical Blocking Tasks (Must Complete First)

**🚨 BLOCKER 1: Database Schema Migration**
- **Owner**: Agent A (Database Focus)
- **Priority**: Highest
- **Blocks**: Everything else depends on these tables existing
- **Deliverable**: Create and apply migration for `magic_links` and `sessions` tables

**🚨 BLOCKER 2: Session Management Infrastructure** 
- **Owner**: Agent A (Database Focus)  
- **Priority**: Highest
- **Blocks**: Authentication handlers, middleware updates
- **Deliverable**: PostgreSQL session storage, cookie handling, session cleanup

### Parallel Execution Phase (After Blockers Complete)

#### Agent A: Infrastructure & Database Track
**Focus**: Core authentication infrastructure, database operations, middleware

**Task A1: Password System Cleanup**
- **Dependencies**: None (independent cleanup)
- **Parallel with**: Agent B's SES integration
- **Deliverables**:
  - Remove password columns from `identities` table
  - Remove Argon2 password hashing code  
  - Remove password validation logic
  - Clean up password-related API endpoints
  - Update user registration (no password required)

**Task A2: Authentication Middleware Updates**  
- **Dependencies**: Session management infrastructure (BLOCKER 2)
- **Parallel with**: Agent B's magic link service
- **Deliverables**:
  - Replace JWT middleware with session-based auth
  - Session cookie management middleware
  - Update rate limiting for magic link endpoints
  - Session-based route protection

#### Agent B: Email & Authentication Logic Track
**Focus**: External integrations, user-facing authentication features

**Task B1: AWS SES Integration**
- **Dependencies**: None (DevOps setup complete)
- **Parallel with**: Agent A's password cleanup
- **Deliverables**:
  - SES service integration using provided IAM/config
  - Email templates for magic links
  - Error handling and retry logic
  - Integration with telemetry/logging

**Task B2: Magic Link Service Implementation**
- **Dependencies**: Database migration (BLOCKER 1) + SES integration (Task B1)
- **Parallel with**: Agent A's middleware updates
- **Deliverables**:
  - Token generation (32-byte CSPRNG → base64url)
  - SHA-256 hash storage (never store plaintext tokens)
  - CSRF state parameter handling
  - Rate limiting (3 emails/hour per address)
  - Token validation and consumption logic

### Integration Phase (Both Agents)

**Joint Task: Authentication API Endpoints**
- **Dependencies**: All parallel tasks complete
- **Approach**: Pair programming or review handoff
- **Deliverables**:
  - `POST /auth/magic-link` - Request magic link
  - `POST /auth/verify` - Verify token and create session  
  - `GET /auth/me` - Session-based user info
  - `POST /auth/logout` - Session cleanup
  - Remove old password-based login endpoints

### Final Tasks (Can be Split)

**Testing & Validation**
- **Agent A**: Database integration tests, session management tests
- **Agent B**: Email integration tests, magic link service tests
- **Joint**: End-to-end authentication flow testing

**Documentation Updates**
- **Agent A**: Database schema docs, session management docs
- **Agent B**: Email service docs, magic link flow docs  
- **Joint**: OpenAPI/Swagger spec updates

### Execution Timeline

```
Week 1:
├── Agent A: Database migration + Session infrastructure (BLOCKERS)
└── Agent B: SES integration setup

Week 2:  
├── Agent A: Password cleanup + Middleware updates
└── Agent B: Magic link service implementation

Week 3:
├── Joint: Authentication API endpoints
└── Joint: Testing & integration

Week 4:
└── Documentation + final validation
```

### Key Coordination Points

1. **After BLOCKER completion**: Both agents sync on database schema and session interface
2. **Before integration phase**: Code review of all parallel work
3. **During API implementation**: Define clear interfaces between session management and magic link services
4. **Testing coordination**: Ensure test data setup works for both tracks

## Original Implementation Plan (Reference)

### Phase 1: Infrastructure Setup

#### Task 1.1: AWS SES Integration
**Priority**: High  
**Dependencies**: AWS IAM setup (being handled separately)  
**Deliverables**:
- [ ] AWS SES service integration in `api/src/`
- [ ] Basic email template for magic links
- [ ] Configuration management for SES settings
- [ ] Email sending service with basic error handling
- [ ] Integration with existing telemetry/logging

**Implementation Details**:
- Add `aws-config` and `aws-sdk-sesv2` dependencies
- Create `EmailService` trait and SES implementation
- Basic HTML email template (can iterate later)
- Environment variables: `SES_REGION`, `EMAIL_FROM`, `SES_CONFIGURATION_SET`
- Error handling for SES API failures

#### Task 1.2: Session Management Infrastructure
**Priority**: High  
**Dependencies**: None  
**Deliverables**:
- [ ] Session storage system (PostgreSQL-backed)
- [ ] Session cookie management middleware
- [ ] Session cleanup/expiry handling
- [ ] Session-based authentication middleware

**Implementation Details**:
- New `sessions` table in PostgreSQL
- Signed, encrypted session cookies (HttpOnly, Secure, SameSite=Lax)
- Background cleanup job for expired sessions
- Replace JWT middleware with session middleware for protected routes

#### Task 1.3: Password System Cleanup
**Priority**: High  
**Dependencies**: None  
**Deliverables**:
- [ ] Remove password-related fields from database schema
- [ ] Remove Argon2 password hashing code
- [ ] Remove password validation logic
- [ ] Clean up password-related API endpoints
- [ ] Update user registration to not require passwords

**Implementation Details**:
- Remove `password_hash`, `password_reset_token`, `password_reset_expires_at` columns
- Remove password validation functions
- Update user creation endpoints
- Clean up auth service password methods

### Phase 2: Database Schema Changes

#### Task 2.1: Magic Link and Session Tables
**Priority**: High  
**Dependencies**: None  
**Deliverables**:
- [ ] Create `magic_links` table
- [ ] Create `sessions` table  
- [ ] Remove password-related columns from `identities`
- [ ] SQLX migration file for deployment

**Database Schema**:
```sql
-- Remove password fields from identities table
ALTER TABLE identities DROP COLUMN IF EXISTS password_hash;
ALTER TABLE identities DROP COLUMN IF EXISTS password_reset_token;
ALTER TABLE identities DROP COLUMN IF EXISTS password_reset_expires_at;

-- Magic links table
CREATE TABLE magic_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id),
    token_hash BYTEA NOT NULL, -- SHA-256 of the actual token
    state VARCHAR(32) NOT NULL, -- CSRF protection nonce
    redirect_uri VARCHAR(500),
    client_ip INET,
    user_agent TEXT,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    consumed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for magic_links
CREATE UNIQUE INDEX idx_magic_links_token_hash ON magic_links(token_hash);
CREATE INDEX idx_magic_links_user_id ON magic_links(user_id);
CREATE INDEX idx_magic_links_expires_at ON magic_links(expires_at);

-- Sessions table
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES identities(id),
    session_token VARCHAR(64) NOT NULL UNIQUE,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    client_ip INET,
    user_agent TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    last_accessed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Indexes for sessions
CREATE UNIQUE INDEX idx_sessions_token ON sessions(session_token);
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);
```

### Phase 3: Backend API Implementation

#### Task 3.1: Magic Link Service
**Priority**: High  
**Dependencies**: Task 1.1, Task 2.1  
**Deliverables**:
- [ ] `MagicLinkService` struct with token generation
- [ ] Email composition and sending via SES
- [ ] Token validation and consumption
- [ ] Basic rate limiting (in-memory)

**Implementation Details**:
- 32-byte CSPRNG tokens → base64url encoded
- Store only SHA-256 hash of token in database
- Generate CSRF state nonce (16 bytes)
- Basic email template
- TTL: 10 minutes, single-use tokens
- In-memory rate limiting: 3 emails per hour per address

#### Task 3.2: Authentication Handlers
**Priority**: High  
**Dependencies**: Task 3.1, Task 1.2  
**Deliverables**:
- [ ] `POST /auth/magic-link` endpoint implementation
- [ ] `POST /auth/verify` endpoint implementation
- [ ] Updated `GET /auth/me` for session-based auth
- [ ] Updated `POST /auth/logout` for session cleanup
- [ ] Remove old password-based login endpoints

**Implementation Details**:
- Basic redirect URI validation
- Audit logging for authentication events
- Error handling with appropriate status codes
- OpenAPI documentation compliance

#### Task 3.3: Middleware Updates
**Priority**: High  
**Dependencies**: Task 1.2  
**Deliverables**:
- [ ] Session-based authentication middleware (replace JWT)
- [ ] Session cookie management
- [ ] Update existing rate limiting for magic links
- [ ] Clean up password-related middleware

### Phase 4: Testing & Deployment

#### Task 4.1: Database Migration
**Priority**: High  
**Dependencies**: Phase 2 complete  
**Deliverables**:
- [ ] SQLX migration file for new tables and schema cleanup
- [ ] Basic rollback plan (if needed)

**Implementation Details**:
- Single migration file to create magic_links and sessions tables
- Remove password columns from identities table
- Apply via existing SQLX migration system

#### Task 4.2: Testing
**Priority**: High  
**Dependencies**: Phase 3 complete  
**Deliverables**:
- [ ] Unit tests for MagicLinkService
- [ ] Unit tests for authentication handlers
- [ ] Integration tests for complete auth flow
- [ ] Database integration tests

**Implementation Details**:
- Test token generation and validation
- Test email sending (mock SES in tests)
- Test session creation and validation
- Test rate limiting behavior

#### Task 4.3: Documentation Updates
**Priority**: Medium  
**Dependencies**: All implementation tasks  
**Deliverables**:
- [ ] OpenAPI/Swagger spec updates for new endpoints
- [ ] Update api/README.md with new auth flow
- [ ] Update any relevant documentation in docs/

**Implementation Details**:
- Document all new endpoints with proper OpenAPI annotations
- Update authentication integration guides
- Remove password-based auth references

## Technical Specifications

### Security Considerations
- **Token Generation**: 32 bytes CSPRNG → base64url (no padding)
- **Storage**: Only SHA-256 hashes stored in database
- **TTL**: 10 minutes, single-use tokens
- **Rate Limiting**: 3 emails/hour per address (in-memory, simple implementation)
- **CSRF Protection**: State parameter validation
- **Session Security**: HttpOnly, Secure, SameSite=Lax cookies
- **Basic Redirect Validation**: Simple allowlist for common paths

### Configuration
```env
# AWS SES Configuration (being set up separately via IAM)
SES_REGION=us-east-1
EMAIL_FROM=auth@arda.xyz
EMAIL_REPLY_TO=support@arda.xyz
SES_CONFIGURATION_SET=tx-auth

# Magic Link Settings
LINK_TTL_MINUTES=10
MAGIC_LINK_RATE_LIMIT_PER_EMAIL=3

# Session Settings
SESSION_EXPIRY_DAYS=30
SESSION_COOKIE_DOMAIN=.arda.xyz
```

### Dependencies to Add
```toml
# Backend (api/Cargo.toml)
aws-config = "1.0"
aws-sdk-sesv2 = "1.0" 
tower-cookies = "0.10"
base64 = "0.22"
sha2 = "0.10"
rand = "0.8"
```

## Risk Assessment

### High Risk Items
- **AWS SES Configuration**: Requires domain verification and proper IAM setup (being handled separately)
- **Database Migration**: Schema changes and password field cleanup
- **Complete Auth System Replacement**: Hard cutover without fallback

### Medium Risk Items
- **Email Deliverability**: Basic implementation may hit spam filters
- **Session Management**: New cookie-based system replacing JWT

### Low Risk Items
- **Token Generation**: Well-established cryptographic patterns
- **Rate Limiting**: Simple in-memory implementation for beta usage
- **Testing**: Standard patterns can be followed

## Success Criteria

### Functional Requirements
- [ ] Users can request magic links via email
- [ ] Magic links work reliably with proper security
- [ ] Session-based authentication functions correctly
- [ ] Email delivery works for basic testing
- [ ] Simple rate limiting prevents basic abuse
- [ ] All existing API functionality remains intact (with new auth)

### Non-Functional Requirements
- [ ] Authentication flow works end-to-end
- [ ] Database migration completes successfully
- [ ] No data loss during password field removal
- [ ] OpenAPI documentation is accurate
- [ ] Basic load testing passes for beta usage

## Implementation Specifications

### Security Policy Configuration
**Redirect URI Allowlist**: Restrictive approach
```rust
let allowed_redirects = vec![
    "/dashboard",
    "/onboarding", 
    "/settings",
    "/profile"
];
```

**Error Messages**: User-friendly with specific messages
```rust
match error_type {
    TokenExpired => "This link has expired. Please request a new one.",
    TokenUsed => "This link has already been used. Please request a new one.",
    TokenInvalid => "Invalid authentication link. Please request a new one.",
    StateInvalid => "Security validation failed. Please try again.",
}
```

**CORS/Cookie Policy**: Basic same-site configuration
```rust
CookieBuilder::new("session_token", session_id)
    .same_site(SameSite::Lax)
    .secure(true)
    .http_only(true)
    .path("/")
    .max_age(Duration::days(30))
```

### Email Configuration
**Magic Link URL Format**:
```
https://app.arda.xyz/auth/verify?token=abc123...&state=xyz789...
```

**Email From Address**:
```
From: "Arda Credit" <auth@arda.xyz>
```

**Email Content Template** (Plain Text):
```
Subject: Your Arda Credit login link

Click this link to sign in to your Arda Credit account:
https://app.arda.xyz/auth/verify?token=...&state=...

This link expires in 10 minutes.

If you didn't request this, please ignore this email.
```

### Implementation Priority
**Phase Order**: Follow numbered sequence (1 → 2 → 3 → 4)
**Task Order**: Infrastructure First approach within each phase

---

**Next Steps**: All specifications are now defined. Ready to create detailed implementation prompts for each task.

## Summary

This simplified plan focuses on a **complete cutover** from password-based to magic link authentication for the beta environment:

- **4 Phases, 10 Tasks** (down from 6 phases, 24 tasks)
- **Backend-only implementation** (frontend handled separately)
- **PostgreSQL-based** (no Redis complexity)
- **In-memory rate limiting** (sufficient for beta load)
- **Basic AWS SES integration** (IAM setup handled separately)
- **Hard migration** (no backwards compatibility needed)
- **Standard testing approach** (unit + integration tests)

The implementation removes all password logic and replaces it with a secure, session-based magic link system suitable for proof-of-concept deployment.
