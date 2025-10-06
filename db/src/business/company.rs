//! Company Access Control Module
//!
//! This module provides types and functions for company-based access control.
//! Companies can have admins and regular users (investors/originators) with
//! granular role-based permissions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{DbError, DbResult};

// ============================================================================
// CORE TYPES
// ============================================================================

/// Company/organization that manages a team of users
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Company {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: CompanyStatus,
    #[sqlx(default)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Company status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum CompanyStatus {
    Active,
    Suspended,
    Inactive,
}

impl std::fmt::Display for CompanyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Inactive => write!(f, "inactive"),
        }
    }
}

impl std::str::FromStr for CompanyStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "inactive" => Ok(Self::Inactive),
            _ => Err(format!("Invalid company status: {}", s)),
        }
    }
}

/// User role within a company
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum CompanyRole {
    Admin, // Full permissions
    User,  // Limited permissions
}

impl std::fmt::Display for CompanyRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::User => write!(f, "user"),
        }
    }
}

impl std::str::FromStr for CompanyRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Self::Admin),
            "user" => Ok(Self::User),
            _ => Err(format!("Invalid company role: {}", s)),
        }
    }
}

/// Membership status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum MembershipStatus {
    Pending,
    Active,
    Suspended,
    Removed,
}

impl std::fmt::Display for MembershipStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Removed => write!(f, "removed"),
        }
    }
}

impl std::str::FromStr for MembershipStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "removed" => Ok(Self::Removed),
            _ => Err(format!("Invalid membership status: {}", s)),
        }
    }
}

/// Company membership linking users to companies with roles
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyMembership {
    pub id: Uuid,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: CompanyRole,
    pub status: MembershipStatus,
    pub invited_by: Option<Uuid>,
    pub invited_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub permissions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Invitation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Rejected,
    Expired,
    Cancelled,
}

impl std::fmt::Display for InvitationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Accepted => write!(f, "accepted"),
            Self::Rejected => write!(f, "rejected"),
            Self::Expired => write!(f, "expired"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for InvitationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "expired" => Ok(Self::Expired),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Invalid invitation status: {}", s)),
        }
    }
}

/// Company invitation for new members
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyInvitation {
    pub id: Uuid,
    pub company_id: Uuid,
    pub email: String,
    pub role: CompanyRole,
    pub invited_by: Uuid,
    pub invitation_token: String,
    pub status: InvitationStatus,
    pub expires_at: DateTime<Utc>,
    pub accepted_by: Option<Uuid>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Audit trail for company operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyAudit {
    pub id: Uuid,
    pub company_id: Uuid,
    pub operation_type: String,
    pub operator_id: Option<Uuid>,
    pub target_user_id: Option<Uuid>,
    #[sqlx(default)]
    pub old_values: Option<serde_json::Value>,
    #[sqlx(default)]
    pub new_values: Option<serde_json::Value>,
    #[sqlx(default)]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Request to create a new company
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCompanyRequest {
    pub name: String,
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to update a company
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCompanyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<CompanyStatus>,
    pub metadata: Option<serde_json::Value>,
}

/// Request to invite a user to a company
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteUserRequest {
    pub email: String,
    pub role: CompanyRole,
    pub invited_by: Uuid,
}

/// Request to update a membership
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMembershipRequest {
    pub role: Option<CompanyRole>,
    pub status: Option<MembershipStatus>,
    pub permissions: Option<serde_json::Value>,
}

/// Request to accept an invitation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptInvitationRequest {
    pub invitation_token: String,
    pub user_id: Uuid,
}

/// Company with membership count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyWithStats {
    pub company: Company,
    pub member_count: i64,
    pub admin_count: i64,
    pub active_deal_count: i64,
}

/// Membership with user details (flat structure for SQL compatibility)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MembershipWithUser {
    // Membership fields
    pub id: Uuid,
    pub company_id: Uuid,
    pub user_id: Uuid,
    pub role: CompanyRole,
    pub status: MembershipStatus,
    pub invited_by: Option<Uuid>,
    pub invited_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    #[sqlx(default)]
    pub permissions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    // User fields
    pub user_email: String,
    pub user_type: String,
    pub kyc_status: String,
}

impl MembershipWithUser {
    /// Convert to separate membership and user details
    pub fn to_membership(&self) -> CompanyMembership {
        CompanyMembership {
            id: self.id,
            company_id: self.company_id,
            user_id: self.user_id,
            role: self.role,
            status: self.status,
            invited_by: self.invited_by,
            invited_at: self.invited_at,
            accepted_at: self.accepted_at,
            permissions: self.permissions.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

// ============================================================================
// PERMISSION DEFINITIONS
// ============================================================================

/// Standard permissions for company members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyPermissions {
    pub can_create_deals: bool,
    pub can_approve_deals: bool,
    pub can_reject_deals: bool,
    pub can_cancel_deals: bool,
    pub can_invite_users: bool,
    pub can_remove_users: bool,
    pub can_approve_documents: bool,
    pub can_reject_documents: bool,
    pub can_manage_company: bool,
}

impl CompanyPermissions {
    /// Default permissions for admin role
    pub fn admin_default() -> Self {
        Self {
            can_create_deals: true,
            can_approve_deals: true,
            can_reject_deals: true,
            can_cancel_deals: true,
            can_invite_users: true,
            can_remove_users: true,
            can_approve_documents: true,
            can_reject_documents: true,
            can_manage_company: true,
        }
    }

    /// Default permissions for user role
    pub fn user_default() -> Self {
        Self {
            can_create_deals: false,
            can_approve_deals: false,
            can_reject_deals: false,
            can_cancel_deals: false,
            can_invite_users: false,
            can_remove_users: false,
            can_approve_documents: false,
            can_reject_documents: false,
            can_manage_company: false,
        }
    }

    /// Convert to JSON value
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_default()
    }

    /// Create from JSON value
    pub fn from_json(value: &serde_json::Value) -> DbResult<Self> {
        serde_json::from_value(value.clone())
            .map_err(|e| DbError::InvalidState(format!("Failed to parse permissions: {}", e)))
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Generate a secure random invitation token
pub fn generate_invitation_token() -> String {
    use rand::{rng, Rng};

    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut token = String::with_capacity(64);
    let mut rng = rng();

    for _ in 0..64 {
        let idx = rng.random_range(0..CHARSET.len());
        token.push(CHARSET[idx] as char);
    }

    token
}

/// Check if a user has a specific permission in a company
pub fn has_permission(permissions: &serde_json::Value, permission_name: &str) -> bool {
    permissions
        .get(permission_name)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

// ============================================================================
// DATABASE OPERATIONS - COMPANIES
// ============================================================================

/// Create a new company
pub async fn create_company(pool: &PgPool, request: &CreateCompanyRequest) -> DbResult<Company> {
    let metadata = request
        .metadata
        .clone()
        .unwrap_or_else(|| serde_json::json!({}));

    let company = sqlx::query_as::<_, Company>(
        r#"
        INSERT INTO companies (name, description, metadata, status)
        VALUES ($1, $2, $3, 'active')
        RETURNING id, name, description, status, metadata, created_at, updated_at
        "#,
    )
    .bind(&request.name)
    .bind(&request.description)
    .bind(&metadata)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to create company: {}", e)))?;

    Ok(company)
}

/// Get company by ID
pub async fn get_company_by_id(pool: &PgPool, company_id: Uuid) -> DbResult<Company> {
    let company = sqlx::query_as::<_, Company>(
        r#"
        SELECT id, name, description, status, metadata, created_at, updated_at
        FROM companies
        WHERE id = $1
        "#,
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Company not found: {}", e)))?;

    Ok(company)
}

/// List all companies with optional status filter
pub async fn list_companies(
    pool: &PgPool,
    status: Option<CompanyStatus>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> DbResult<Vec<Company>> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let companies = match status {
        Some(status) => {
            sqlx::query_as::<_, Company>(
                r#"
                SELECT id, name, description, status, metadata, created_at, updated_at
                FROM companies
                WHERE status = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(status.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, Company>(
                r#"
                SELECT id, name, description, status, metadata, created_at, updated_at
                FROM companies
                ORDER BY created_at DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| DbError::TransactionFailed(format!("Failed to list companies: {}", e)))?;

    Ok(companies)
}

/// Update a company
pub async fn update_company(
    pool: &PgPool,
    company_id: Uuid,
    request: &UpdateCompanyRequest,
) -> DbResult<Company> {
    // Build dynamic update query
    let mut updates = Vec::new();
    let mut param_count = 2; // Starting from $2 since $1 is company_id

    if request.name.is_some() {
        updates.push(format!("name = ${}", param_count));
        param_count += 1;
    }
    if request.description.is_some() {
        updates.push(format!("description = ${}", param_count));
        param_count += 1;
    }
    if request.status.is_some() {
        updates.push(format!("status = ${}", param_count));
        param_count += 1;
    }
    if request.metadata.is_some() {
        updates.push(format!("metadata = ${}", param_count));
    }

    if updates.is_empty() {
        return get_company_by_id(pool, company_id).await;
    }

    let query_str = format!(
        r#"
        UPDATE companies
        SET {}
        WHERE id = $1
        RETURNING id, name, description, status, metadata, created_at, updated_at
        "#,
        updates.join(", ")
    );

    let mut query = sqlx::query_as::<_, Company>(&query_str).bind(company_id);

    if let Some(name) = &request.name {
        query = query.bind(name);
    }
    if let Some(description) = &request.description {
        query = query.bind(description);
    }
    if let Some(status) = &request.status {
        query = query.bind(status.to_string());
    }
    if let Some(metadata) = &request.metadata {
        query = query.bind(metadata);
    }

    let company = query
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to update company: {}", e)))?;

    Ok(company)
}

/// Delete a company (soft delete by setting status to inactive)
pub async fn delete_company(pool: &PgPool, company_id: Uuid) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE companies
        SET status = 'inactive'
        WHERE id = $1
        "#,
    )
    .bind(company_id)
    .execute(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to delete company: {}", e)))?;

    Ok(())
}

/// Get company with statistics
pub async fn get_company_with_stats(pool: &PgPool, company_id: Uuid) -> DbResult<CompanyWithStats> {
    let company = get_company_by_id(pool, company_id).await?;

    let stats = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"
        SELECT
            COUNT(DISTINCT cm.id) FILTER (WHERE cm.status = 'active') as member_count,
            COUNT(DISTINCT cm.id) FILTER (WHERE cm.status = 'active' AND cm.role = 'admin') as admin_count,
            COUNT(DISTINCT d.id) FILTER (WHERE d.loan_status IN ('originated', 'active')) as active_deal_count
        FROM companies c
        LEFT JOIN company_memberships cm ON c.id = cm.company_id
        LEFT JOIN deals d ON c.id = d.company_id
        WHERE c.id = $1
        "#,
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to get company stats: {}", e)))?;

    Ok(CompanyWithStats {
        company,
        member_count: stats.0,
        admin_count: stats.1,
        active_deal_count: stats.2,
    })
}

// ============================================================================
// DATABASE OPERATIONS - MEMBERSHIPS
// ============================================================================

/// Create a company membership
pub async fn create_membership(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    role: CompanyRole,
    invited_by: Option<Uuid>,
    permissions: Option<serde_json::Value>,
) -> DbResult<CompanyMembership> {
    let permissions = permissions.unwrap_or_else(|| match role {
        CompanyRole::Admin => CompanyPermissions::admin_default().to_json(),
        CompanyRole::User => CompanyPermissions::user_default().to_json(),
    });

    let membership = sqlx::query_as::<_, CompanyMembership>(
        r#"
        INSERT INTO company_memberships (company_id, user_id, role, status, invited_by, permissions)
        VALUES ($1, $2, $3, 'pending', $4, $5)
        RETURNING id, company_id, user_id, role, status, invited_by, invited_at, accepted_at, permissions, created_at, updated_at
        "#,
    )
    .bind(company_id)
    .bind(user_id)
    .bind(role.to_string())
    .bind(invited_by)
    .bind(&permissions)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to create membership: {}", e)))?;

    Ok(membership)
}

/// Get membership by ID
pub async fn get_membership_by_id(
    pool: &PgPool,
    membership_id: Uuid,
) -> DbResult<CompanyMembership> {
    let membership = sqlx::query_as::<_, CompanyMembership>(
        r#"
        SELECT id, company_id, user_id, role, status, invited_by, invited_at, accepted_at, permissions, created_at, updated_at
        FROM company_memberships
        WHERE id = $1
        "#,
    )
    .bind(membership_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Membership not found: {}", e)))?;

    Ok(membership)
}

/// Get user's membership in a company
pub async fn get_user_membership(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
) -> DbResult<CompanyMembership> {
    let membership = sqlx::query_as::<_, CompanyMembership>(
        r#"
        SELECT id, company_id, user_id, role, status, invited_by, invited_at, accepted_at, permissions, created_at, updated_at
        FROM company_memberships
        WHERE company_id = $1 AND user_id = $2
        "#,
    )
    .bind(company_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Membership not found: {}", e)))?;

    Ok(membership)
}

/// List company members with optional filters
pub async fn list_company_members(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<MembershipStatus>,
    role: Option<CompanyRole>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> DbResult<Vec<MembershipWithUser>> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let members = match (status, role) {
        (Some(status), Some(role)) => {
            sqlx::query_as::<_, MembershipWithUser>(
                r#"
                SELECT
                    cm.id, cm.company_id, cm.user_id, cm.role, cm.status,
                    cm.invited_by, cm.invited_at, cm.accepted_at, cm.permissions,
                    cm.created_at, cm.updated_at,
                    u.email as user_email, u.user_type, u.kyc_status
                FROM company_memberships cm
                JOIN users u ON cm.user_id = u.id
                WHERE cm.company_id = $1 AND cm.status = $2 AND cm.role = $3
                ORDER BY cm.created_at DESC
                LIMIT $4 OFFSET $5
                "#,
            )
            .bind(company_id)
            .bind(status.to_string())
            .bind(role.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (Some(status), None) => {
            sqlx::query_as::<_, MembershipWithUser>(
                r#"
                SELECT
                    cm.id, cm.company_id, cm.user_id, cm.role, cm.status,
                    cm.invited_by, cm.invited_at, cm.accepted_at, cm.permissions,
                    cm.created_at, cm.updated_at,
                    u.email as user_email, u.user_type, u.kyc_status
                FROM company_memberships cm
                JOIN users u ON cm.user_id = u.id
                WHERE cm.company_id = $1 AND cm.status = $2
                ORDER BY cm.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(company_id)
            .bind(status.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (None, Some(role)) => {
            sqlx::query_as::<_, MembershipWithUser>(
                r#"
                SELECT
                    cm.id, cm.company_id, cm.user_id, cm.role, cm.status,
                    cm.invited_by, cm.invited_at, cm.accepted_at, cm.permissions,
                    cm.created_at, cm.updated_at,
                    u.email as user_email, u.user_type, u.kyc_status
                FROM company_memberships cm
                JOIN users u ON cm.user_id = u.id
                WHERE cm.company_id = $1 AND cm.role = $2
                ORDER BY cm.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(company_id)
            .bind(role.to_string())
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        (None, None) => {
            sqlx::query_as::<_, MembershipWithUser>(
                r#"
                SELECT
                    cm.id, cm.company_id, cm.user_id, cm.role, cm.status,
                    cm.invited_by, cm.invited_at, cm.accepted_at, cm.permissions,
                    cm.created_at, cm.updated_at,
                    u.email as user_email, u.user_type, u.kyc_status
                FROM company_memberships cm
                JOIN users u ON cm.user_id = u.id
                WHERE cm.company_id = $1
                ORDER BY cm.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(company_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| DbError::TransactionFailed(format!("Failed to list company members: {}", e)))?;

    Ok(members)
}

/// Update a membership
pub async fn update_membership(
    pool: &PgPool,
    membership_id: Uuid,
    request: &UpdateMembershipRequest,
) -> DbResult<CompanyMembership> {
    let mut updates = Vec::new();
    let mut param_count = 2;

    if request.role.is_some() {
        updates.push(format!("role = ${}", param_count));
        param_count += 1;
    }
    if request.status.is_some() {
        updates.push(format!("status = ${}", param_count));
        param_count += 1;
    }
    if request.permissions.is_some() {
        updates.push(format!("permissions = ${}", param_count));
    }

    if updates.is_empty() {
        return get_membership_by_id(pool, membership_id).await;
    }

    let query_str = format!(
        r#"
        UPDATE company_memberships
        SET {}
        WHERE id = $1
        RETURNING id, company_id, user_id, role, status, invited_by, invited_at, accepted_at, permissions, created_at, updated_at
        "#,
        updates.join(", ")
    );

    let mut query = sqlx::query_as::<_, CompanyMembership>(&query_str).bind(membership_id);

    if let Some(role) = &request.role {
        query = query.bind(role.to_string());
    }
    if let Some(status) = &request.status {
        query = query.bind(status.to_string());
    }
    if let Some(permissions) = &request.permissions {
        query = query.bind(permissions);
    }

    let membership = query
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to update membership: {}", e)))?;

    Ok(membership)
}

/// Accept a membership invitation
pub async fn accept_membership(pool: &PgPool, membership_id: Uuid) -> DbResult<CompanyMembership> {
    let membership = sqlx::query_as::<_, CompanyMembership>(
        r#"
        UPDATE company_memberships
        SET status = 'active', accepted_at = NOW()
        WHERE id = $1
        RETURNING id, company_id, user_id, role, status, invited_by, invited_at, accepted_at, permissions, created_at, updated_at
        "#,
    )
    .bind(membership_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to accept membership: {}", e)))?;

    Ok(membership)
}

/// Remove a member from a company
pub async fn remove_membership(pool: &PgPool, membership_id: Uuid) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE company_memberships
        SET status = 'removed'
        WHERE id = $1
        "#,
    )
    .bind(membership_id)
    .execute(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to remove membership: {}", e)))?;

    Ok(())
}

// ============================================================================
// DATABASE OPERATIONS - INVITATIONS
// ============================================================================

/// Create a company invitation
pub async fn create_invitation(
    pool: &PgPool,
    request: &InviteUserRequest,
    company_id: Uuid,
) -> DbResult<CompanyInvitation> {
    let invitation_token = generate_invitation_token();

    let invitation = sqlx::query_as::<_, CompanyInvitation>(
        r#"
        INSERT INTO company_invitations (company_id, email, role, invited_by, invitation_token)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, company_id, email, role, invited_by, invitation_token, status, expires_at, accepted_by, accepted_at, created_at, updated_at
        "#,
    )
    .bind(company_id)
    .bind(&request.email)
    .bind(request.role.to_string())
    .bind(request.invited_by)
    .bind(&invitation_token)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to create invitation: {}", e)))?;

    Ok(invitation)
}

/// Get invitation by token
pub async fn get_invitation_by_token(pool: &PgPool, token: &str) -> DbResult<CompanyInvitation> {
    let invitation = sqlx::query_as::<_, CompanyInvitation>(
        r#"
        SELECT id, company_id, email, role, invited_by, invitation_token, status, expires_at, accepted_by, accepted_at, created_at, updated_at
        FROM company_invitations
        WHERE invitation_token = $1 AND status = 'pending' AND expires_at > NOW()
        "#,
    )
    .bind(token)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::NotFound(format!("Invitation not found or expired: {}", e)))?;

    Ok(invitation)
}

/// List company invitations
pub async fn list_company_invitations(
    pool: &PgPool,
    company_id: Uuid,
    status: Option<InvitationStatus>,
) -> DbResult<Vec<CompanyInvitation>> {
    let invitations = match status {
        Some(status) => {
            sqlx::query_as::<_, CompanyInvitation>(
                r#"
                SELECT id, company_id, email, role, invited_by, invitation_token, status, expires_at, accepted_by, accepted_at, created_at, updated_at
                FROM company_invitations
                WHERE company_id = $1 AND status = $2
                ORDER BY created_at DESC
                "#,
            )
            .bind(company_id)
            .bind(status.to_string())
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query_as::<_, CompanyInvitation>(
                r#"
                SELECT id, company_id, email, role, invited_by, invitation_token, status, expires_at, accepted_by, accepted_at, created_at, updated_at
                FROM company_invitations
                WHERE company_id = $1
                ORDER BY created_at DESC
                "#,
            )
            .bind(company_id)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| DbError::TransactionFailed(format!("Failed to list invitations: {}", e)))?;

    Ok(invitations)
}

/// Accept an invitation and create membership
pub async fn accept_invitation(
    pool: &PgPool,
    request: &AcceptInvitationRequest,
) -> DbResult<CompanyMembership> {
    let invitation = get_invitation_by_token(pool, &request.invitation_token).await?;

    // Start transaction
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to start transaction: {}", e)))?;

    // Update invitation status
    sqlx::query(
        r#"
        UPDATE company_invitations
        SET status = 'accepted', accepted_by = $1, accepted_at = NOW()
        WHERE invitation_token = $2
        "#,
    )
    .bind(request.user_id)
    .bind(&request.invitation_token)
    .execute(&mut *tx)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to update invitation: {}", e)))?;

    // Create membership
    let permissions = match invitation.role {
        CompanyRole::Admin => CompanyPermissions::admin_default().to_json(),
        CompanyRole::User => CompanyPermissions::user_default().to_json(),
    };

    let membership = sqlx::query_as::<_, CompanyMembership>(
        r#"
        INSERT INTO company_memberships (company_id, user_id, role, status, invited_by, accepted_at, permissions)
        VALUES ($1, $2, $3, 'active', $4, NOW(), $5)
        ON CONFLICT (company_id, user_id)
        DO UPDATE SET status = 'active', accepted_at = NOW(), role = EXCLUDED.role, permissions = EXCLUDED.permissions
        RETURNING id, company_id, user_id, role, status, invited_by, invited_at, accepted_at, permissions, created_at, updated_at
        "#,
    )
    .bind(invitation.company_id)
    .bind(request.user_id)
    .bind(invitation.role.to_string())
    .bind(invitation.invited_by)
    .bind(&permissions)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to create membership: {}", e)))?;

    // Commit transaction
    tx.commit()
        .await
        .map_err(|e| DbError::TransactionFailed(format!("Failed to commit transaction: {}", e)))?;

    Ok(membership)
}

/// Cancel an invitation
pub async fn cancel_invitation(pool: &PgPool, invitation_id: Uuid) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE company_invitations
        SET status = 'cancelled'
        WHERE id = $1
        "#,
    )
    .bind(invitation_id)
    .execute(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to cancel invitation: {}", e)))?;

    Ok(())
}

// ============================================================================
// AUTHORIZATION HELPERS
// ============================================================================

/// Check if user is admin of a company
pub async fn is_company_admin(pool: &PgPool, user_id: Uuid, company_id: Uuid) -> DbResult<bool> {
    let result = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM company_memberships
            WHERE user_id = $1 AND company_id = $2 AND role = 'admin' AND status = 'active'
        )
        "#,
    )
    .bind(user_id)
    .bind(company_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to check admin status: {}", e)))?;

    Ok(result)
}

/// Check if user has specific permission in a company
pub async fn check_user_permission(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
    permission: &str,
) -> DbResult<bool> {
    // First check if user is admin (admins have all permissions)
    if is_company_admin(pool, user_id, company_id).await? {
        return Ok(true);
    }

    // Check specific permission
    let membership = match get_user_membership(pool, company_id, user_id).await {
        Ok(m) => m,
        Err(_) => return Ok(false),
    };

    if membership.status != MembershipStatus::Active {
        return Ok(false);
    }

    Ok(has_permission(&membership.permissions, permission))
}

/// Get user's role in a company
pub async fn get_user_role(
    pool: &PgPool,
    user_id: Uuid,
    company_id: Uuid,
) -> DbResult<Option<CompanyRole>> {
    let role = sqlx::query_scalar::<_, Option<String>>(
        r#"
        SELECT role FROM company_memberships
        WHERE user_id = $1 AND company_id = $2 AND status = 'active'
        "#,
    )
    .bind(user_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| DbError::TransactionFailed(format!("Failed to get user role: {}", e)))?;

    match role {
        Some(Some(r)) => Ok(Some(
            r.parse().map_err(|e: String| DbError::InvalidState(e))?,
        )),
        _ => Ok(None),
    }
}
