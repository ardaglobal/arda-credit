//! Company registration and management service
//!
//! This module provides comprehensive company registration, KYB verification,
//! and team management functionality for originators and investors.

use crate::services::auth::Claims;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::fmt;
use thiserror::Error;
use tracing::{error, info};
use uuid::Uuid;
use validator::Validate;

/// Company types in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CompanyType {
    Originator = 1,
    Investor = 2,
}

impl fmt::Display for CompanyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompanyType::Originator => write!(f, "originator"),
            CompanyType::Investor => write!(f, "investor"),
        }
    }
}

/// KYB (Know Your Business) status for company compliance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum KybStatus {
    NotSubmitted = 0,
    Pending = 1,
    Approved = 2,
    Rejected = 3,
}

impl fmt::Display for KybStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KybStatus::NotSubmitted => write!(f, "not_submitted"),
            KybStatus::Pending => write!(f, "pending"),
            KybStatus::Approved => write!(f, "approved"),
            KybStatus::Rejected => write!(f, "rejected"),
        }
    }
}

/// Team member roles within companies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TeamRole {
    Admin = 1, // Full document access, upload, download, delete
    Write = 2, // Upload, download, approve documents, no deletion
    Read = 3,  // View and download only, no upload capabilities
}

impl fmt::Display for TeamRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TeamRole::Admin => write!(f, "admin"),
            TeamRole::Write => write!(f, "write"),
            TeamRole::Read => write!(f, "read"),
        }
    }
}

/// Company service errors
#[derive(Debug, Error)]
pub enum CompanyServiceError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Validation error: {0}")]
    ValidationError(#[from] validator::ValidationErrors),

    #[error("Company not found: {company_id}")]
    CompanyNotFound { company_id: Uuid },

    #[error("Team member not found: user {user_id} in company {company_id}")]
    TeamMemberNotFound { user_id: Uuid, company_id: Uuid },

    #[error("User not found: {user_id}")]
    UserNotFound { user_id: Uuid },

    #[error("Access denied: {message}")]
    AccessDenied { message: String },

    #[error("Company already exists for user: {user_id}")]
    CompanyAlreadyExists { user_id: Uuid },

    #[error("KYB not approved for company: {company_id}")]
    KybNotApproved { company_id: Uuid },

    #[error("Invalid company type: {company_type}")]
    InvalidCompanyType { company_type: String },

    #[error("Team member already exists: user {user_id} in company {company_id}")]
    TeamMemberAlreadyExists { user_id: Uuid, company_id: Uuid },

    #[error("Cannot remove last admin from company: {company_id}")]
    CannotRemoveLastAdmin { company_id: Uuid },

    #[error("Invalid role: {role}")]
    InvalidRole { role: String },
}

/// Company registration request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct CompanyRegistrationRequest {
    #[validate(length(min = 2, max = 100))]
    pub company_name: String,

    #[validate(length(min = 2, max = 50))]
    pub company_type: String, // "originator" or "investor"

    #[validate(length(max = 500))]
    pub description: Option<String>,

    #[validate(length(max = 100))]
    pub industry: Option<String>,

    #[validate(length(max = 100))]
    pub jurisdiction: Option<String>,

    #[validate(length(max = 50))]
    pub registration_number: Option<String>,

    #[validate(email)]
    pub business_email: String,

    #[validate(length(max = 20))]
    pub business_phone: Option<String>,

    pub document_retention_days: Option<u32>,
}

/// Company response DTO
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CompanyResponseDto {
    pub id: Uuid,
    pub company_name: String,
    pub company_type: String,
    pub description: Option<String>,
    pub industry: Option<String>,
    pub jurisdiction: Option<String>,
    pub registration_number: Option<String>,
    pub business_email: String,
    pub business_phone: Option<String>,
    pub kyb_status: String,
    pub kyb_submitted_at: Option<DateTime<Utc>>,
    pub kyb_verified_at: Option<DateTime<Utc>>,
    pub document_retention_days: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub team_member_count: i64,
}

/// Team member invitation request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct TeamInvitationRequest {
    #[validate(email)]
    pub email: String,

    #[validate(length(min = 1, max = 20))]
    pub role: String, // "admin", "write", "read"

    #[validate(length(max = 500))]
    pub invitation_message: Option<String>,

    pub permissions: Option<TeamMemberPermissions>,
}

/// Team member permissions
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TeamMemberPermissions {
    pub can_upload_documents: bool,
    pub can_download_documents: bool,
    pub can_delete_documents: bool,
    pub can_approve_documents: bool,
    pub can_manage_team: bool,
    pub can_view_audit_logs: bool,
}

/// Team member response DTO
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TeamMemberResponseDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub company_id: Uuid,
    pub email: String,
    pub role: String,
    pub status: String,
    pub can_upload_documents: bool,
    pub can_download_documents: bool,
    pub can_delete_documents: bool,
    pub can_approve_documents: bool,
    pub can_manage_team: bool,
    pub can_view_audit_logs: bool,
    pub invited_at: DateTime<Utc>,
    pub joined_at: Option<DateTime<Utc>>,
    pub last_active_at: Option<DateTime<Utc>>,
    pub invited_by: Option<Uuid>,
}

/// KYB submission request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct KybSubmissionRequest {
    #[validate(length(min = 1, max = 100))]
    pub legal_entity_name: String,

    #[validate(length(min = 1, max = 50))]
    pub entity_type: String,

    #[validate(length(min = 1, max = 50))]
    pub jurisdiction: String,

    #[validate(length(min = 1, max = 50))]
    pub registration_number: String,

    pub beneficial_owners: Vec<BeneficialOwnerInfo>,
    pub supporting_documents: Vec<Uuid>, // Document IDs

    #[validate(length(max = 1000))]
    pub additional_notes: Option<String>,
}

/// Beneficial owner information
#[derive(Debug, Clone, Serialize, Deserialize, Validate, utoipa::ToSchema)]
pub struct BeneficialOwnerInfo {
    #[validate(length(min = 1, max = 100))]
    pub full_name: String,

    #[validate(range(min = 0.0, max = 100.0))]
    pub ownership_percentage: f64,

    #[validate(length(max = 100))]
    pub nationality: Option<String>,

    #[validate(length(max = 500))]
    pub address: Option<String>,
}

/// Company service implementation
pub struct CompanyService {
    db_pool: PgPool,
}

impl CompanyService {
    /// Create a new company service
    pub fn new(db_pool: PgPool) -> Self {
        Self { db_pool }
    }

    /// Register a new company
    pub async fn register_company(
        &self,
        claims: &Claims,
        request: CompanyRegistrationRequest,
    ) -> Result<CompanyResponseDto, CompanyServiceError> {
        // Validate request
        request.validate()?;

        // Parse user ID from claims
        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|_| CompanyServiceError::UserNotFound {
                user_id: Uuid::nil(),
            })?;

        // Verify that the user exists in the users table
        let user_exists = sqlx::query!("SELECT id FROM users WHERE id = $1", user_id)
            .fetch_optional(&self.db_pool)
            .await?;

        if user_exists.is_none() {
            return Err(CompanyServiceError::UserNotFound { user_id });
        }

        // Check if user already has a company
        let existing_company =
            sqlx::query!("SELECT id FROM companies WHERE created_by = $1", user_id)
                .fetch_optional(&self.db_pool)
                .await?;

        if existing_company.is_some() {
            return Err(CompanyServiceError::CompanyAlreadyExists { user_id });
        }

        // Parse and validate company type
        let company_type = self.parse_company_type(&request.company_type)?;
        let company_id = Uuid::new_v4();

        // Create company record
        let company = sqlx::query(
            r#"
            INSERT INTO companies (
                id, name, company_name, company_type, description, industry,
                jurisdiction, registration_number, business_email, business_phone,
                kyb_status, document_retention_days, created_by, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW(), NOW()
            ) RETURNING *
            "#,
        )
        .bind(company_id)
        .bind(&request.company_name) // name (required field)
        .bind(&request.company_name) // company_name (optional, same value)
        .bind(company_type.to_string())
        .bind(&request.description)
        .bind(&request.industry)
        .bind(&request.jurisdiction)
        .bind(&request.registration_number)
        .bind(&request.business_email)
        .bind(&request.business_phone)
        .bind(KybStatus::NotSubmitted.to_string())
        .bind(request.document_retention_days.unwrap_or(2555) as i32)
        .bind(user_id)
        .fetch_one(&self.db_pool)
        .await?;

        // Add the creating user as an admin team member
        self.create_team_member(
            company_id,
            user_id,
            TeamRole::Admin,
            true,          // can_upload_documents
            true,          // can_download_documents
            true,          // can_delete_documents
            true,          // can_approve_documents
            true,          // can_manage_team
            true,          // can_view_audit_logs
            Some(user_id), // invited_by (self-added)
        )
        .await?;

        info!(
            company_id = %company_id,
            user_id = %user_id,
            company_name = %request.company_name,
            company_type = %company_type,
            "Company registered successfully"
        );

        Ok(self.row_to_company_dto(company, 1).await)
    }

    /// Get company by ID
    pub async fn get_company(
        &self,
        claims: &Claims,
        company_id: Uuid,
    ) -> Result<CompanyResponseDto, CompanyServiceError> {
        // Parse user ID from claims
        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|_| CompanyServiceError::UserNotFound {
                user_id: Uuid::nil(),
            })?;

        // Check if user is a team member of the company
        self.check_team_membership(user_id, company_id).await?;

        // Get company details
        let company = sqlx::query("SELECT * FROM companies WHERE id = $1")
            .bind(company_id)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or(CompanyServiceError::CompanyNotFound { company_id })?;

        // Get team member count
        let member_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM company_memberships WHERE company_id = $1 AND status = 'active'",
            company_id
        )
        .fetch_one(&self.db_pool)
        .await?
        .count
        .unwrap_or(0);

        Ok(self.row_to_company_dto(company, member_count).await)
    }

    /// Submit KYB documentation
    pub async fn submit_kyb(
        &self,
        claims: &Claims,
        company_id: Uuid,
        request: KybSubmissionRequest,
    ) -> Result<CompanyResponseDto, CompanyServiceError> {
        // Validate request
        request.validate()?;

        // Parse user ID from claims
        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|_| CompanyServiceError::UserNotFound {
                user_id: Uuid::nil(),
            })?;

        // Check if user has admin access to the company
        self.check_company_admin_access(user_id, company_id).await?;

        // Update company with KYB submission
        let company = sqlx::query(
            r#"
            UPDATE companies
            SET
                kyb_status = $1,
                kyb_submitted_at = NOW(),
                legal_entity_name = $2,
                entity_type = $3,
                jurisdiction = $4,
                registration_number = $5,
                beneficial_owners = $6,
                supporting_documents = $7,
                kyb_notes = $8,
                updated_at = NOW()
            WHERE id = $9
            RETURNING *
            "#,
        )
        .bind(KybStatus::Pending.to_string())
        .bind(&request.legal_entity_name)
        .bind(&request.entity_type)
        .bind(&request.jurisdiction)
        .bind(&request.registration_number)
        .bind(serde_json::to_value(&request.beneficial_owners).unwrap())
        .bind(serde_json::to_value(&request.supporting_documents).unwrap())
        .bind(&request.additional_notes)
        .bind(company_id)
        .fetch_one(&self.db_pool)
        .await?;

        // Get team member count
        let member_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM company_memberships WHERE company_id = $1 AND status = 'active'",
            company_id
        )
        .fetch_one(&self.db_pool)
        .await?
        .count
        .unwrap_or(0);

        info!(
            company_id = %company_id,
            user_id = %user_id,
            "KYB documentation submitted"
        );

        Ok(self.row_to_company_dto(company, member_count).await)
    }

    /// Invite team member
    pub async fn invite_team_member(
        &self,
        claims: &Claims,
        company_id: Uuid,
        request: TeamInvitationRequest,
    ) -> Result<TeamMemberResponseDto, CompanyServiceError> {
        // Validate request
        request.validate()?;

        // Parse user ID from claims
        let inviter_id =
            Uuid::parse_str(&claims.sub).map_err(|_| CompanyServiceError::UserNotFound {
                user_id: Uuid::nil(),
            })?;

        // Check if user has team management access
        self.check_team_management_access(inviter_id, company_id)
            .await?;

        // Parse role
        let role = self.parse_team_role(&request.role)?;

        // Check if user already exists, create if not
        let invitee_id = self.get_or_create_user_by_email(&request.email).await?;

        // Check if team member already exists
        let existing_member = sqlx::query!(
            "SELECT id FROM company_memberships WHERE user_id = $1 AND company_id = $2",
            invitee_id,
            company_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        if existing_member.is_some() {
            return Err(CompanyServiceError::TeamMemberAlreadyExists {
                user_id: invitee_id,
                company_id,
            });
        }

        // Set default permissions based on role
        let permissions = request
            .permissions
            .unwrap_or_else(|| self.default_permissions_for_role(&role));

        // Create team member
        let team_member_id = Uuid::new_v4();
        let team_member = sqlx::query(
            r#"
            INSERT INTO team_members (
                id, user_id, company_id, role, status,
                can_upload_documents, can_download_documents, can_delete_documents,
                can_approve_documents, can_manage_team, can_view_audit_logs,
                invited_by, invited_at, invitation_message
            ) VALUES (
                $1, $2, $3, $4, 'invited',
                $5, $6, $7, $8, $9, $10, $11, NOW(), $12
            ) RETURNING *
            "#,
        )
        .bind(team_member_id)
        .bind(invitee_id)
        .bind(company_id)
        .bind(role.to_string())
        .bind(permissions.can_upload_documents)
        .bind(permissions.can_download_documents)
        .bind(permissions.can_delete_documents)
        .bind(permissions.can_approve_documents)
        .bind(permissions.can_manage_team)
        .bind(permissions.can_view_audit_logs)
        .bind(inviter_id)
        .bind(request.invitation_message)
        .fetch_one(&self.db_pool)
        .await?;

        // TODO: Send invitation email

        info!(
            company_id = %company_id,
            inviter_id = %inviter_id,
            invitee_id = %invitee_id,
            role = %role,
            "Team member invited"
        );

        Ok(self.row_to_team_member_dto(team_member).await?)
    }

    /// List team members
    pub async fn list_team_members(
        &self,
        claims: &Claims,
        company_id: Uuid,
    ) -> Result<Vec<TeamMemberResponseDto>, CompanyServiceError> {
        // Parse user ID from claims
        let user_id =
            Uuid::parse_str(&claims.sub).map_err(|_| CompanyServiceError::UserNotFound {
                user_id: Uuid::nil(),
            })?;

        // Check if user is a team member of the company
        self.check_team_membership(user_id, company_id).await?;

        // Get all team members
        let team_members = sqlx::query(
            r#"
            SELECT tm.*, i.email
            FROM company_memberships tm
            JOIN users i ON tm.user_id = i.id
            WHERE tm.company_id = $1
            ORDER BY tm.invited_at DESC
            "#,
        )
        .bind(company_id)
        .fetch_all(&self.db_pool)
        .await?;

        let mut result = Vec::new();
        for member in team_members {
            result.push(self.row_to_team_member_dto(member).await?);
        }

        Ok(result)
    }

    /// Update team member role
    pub async fn update_team_member_role(
        &self,
        claims: &Claims,
        company_id: Uuid,
        member_user_id: Uuid,
        new_role: String,
        permissions: Option<TeamMemberPermissions>,
    ) -> Result<TeamMemberResponseDto, CompanyServiceError> {
        // Parse user ID from claims
        let admin_id =
            Uuid::parse_str(&claims.sub).map_err(|_| CompanyServiceError::UserNotFound {
                user_id: Uuid::nil(),
            })?;

        // Check if user has team management access
        self.check_team_management_access(admin_id, company_id)
            .await?;

        // Parse new role
        let role = self.parse_team_role(&new_role)?;

        // If changing from admin role, ensure not the last admin
        if let Ok(current_member) = self.get_team_member(member_user_id, company_id).await {
            if current_member.role == TeamRole::Admin.to_string() && role != TeamRole::Admin {
                let admin_count = sqlx::query!(
                    "SELECT COUNT(*) as count FROM company_memberships WHERE company_id = $1 AND role = 'admin' AND status = 'active'",
                    company_id
                )
                .fetch_one(&self.db_pool)
                .await?
                .count
                .unwrap_or(0);

                if admin_count <= 1 {
                    return Err(CompanyServiceError::CannotRemoveLastAdmin { company_id });
                }
            }
        }

        // Set permissions based on role if not provided
        let final_permissions =
            permissions.unwrap_or_else(|| self.default_permissions_for_role(&role));

        // Update team member
        let updated_member = sqlx::query(
            r#"
            UPDATE team_members
            SET
                role = $1,
                can_upload_documents = $2,
                can_download_documents = $3,
                can_delete_documents = $4,
                can_approve_documents = $5,
                can_manage_team = $6,
                can_view_audit_logs = $7,
                updated_at = NOW()
            WHERE user_id = $8 AND company_id = $9
            RETURNING *
            "#,
        )
        .bind(role.to_string())
        .bind(final_permissions.can_upload_documents)
        .bind(final_permissions.can_download_documents)
        .bind(final_permissions.can_delete_documents)
        .bind(final_permissions.can_approve_documents)
        .bind(final_permissions.can_manage_team)
        .bind(final_permissions.can_view_audit_logs)
        .bind(member_user_id)
        .bind(company_id)
        .fetch_one(&self.db_pool)
        .await?;

        info!(
            company_id = %company_id,
            admin_id = %admin_id,
            member_user_id = %member_user_id,
            new_role = %role,
            "Team member role updated"
        );

        Ok(self.row_to_team_member_dto(updated_member).await?)
    }

    // Helper methods

    /// Check if user is a team member of the company
    async fn check_team_membership(
        &self,
        user_id: Uuid,
        company_id: Uuid,
    ) -> Result<(), CompanyServiceError> {
        let member = sqlx::query!(
            "SELECT id FROM company_memberships WHERE user_id = $1 AND company_id = $2 AND status = 'active'",
            user_id,
            company_id
        )
        .fetch_optional(&self.db_pool)
        .await?;

        member.ok_or(CompanyServiceError::TeamMemberNotFound {
            user_id,
            company_id,
        })?;
        Ok(())
    }

    /// Check if user has admin access to the company
    async fn check_company_admin_access(
        &self,
        user_id: Uuid,
        company_id: Uuid,
    ) -> Result<(), CompanyServiceError> {
        let member = sqlx::query!(
            "SELECT role FROM company_memberships WHERE user_id = $1 AND company_id = $2 AND status = 'active'",
            user_id,
            company_id
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(CompanyServiceError::TeamMemberNotFound { user_id, company_id })?;

        if member.role != TeamRole::Admin.to_string() {
            return Err(CompanyServiceError::AccessDenied {
                message: "Admin access required".to_string(),
            });
        }

        Ok(())
    }

    /// Check if user has team management access
    async fn check_team_management_access(
        &self,
        user_id: Uuid,
        company_id: Uuid,
    ) -> Result<(), CompanyServiceError> {
        let member = sqlx::query!(
            "SELECT can_manage_team FROM company_memberships WHERE user_id = $1 AND company_id = $2 AND status = 'active'",
            user_id,
            company_id
        )
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(CompanyServiceError::TeamMemberNotFound { user_id, company_id })?;

        if !member.can_manage_team.unwrap_or(false) {
            return Err(CompanyServiceError::AccessDenied {
                message: "Team management access required".to_string(),
            });
        }

        Ok(())
    }

    /// Parse company type from string
    fn parse_company_type(&self, company_type: &str) -> Result<CompanyType, CompanyServiceError> {
        match company_type.to_lowercase().as_str() {
            "originator" => Ok(CompanyType::Originator),
            "investor" => Ok(CompanyType::Investor),
            _ => Err(CompanyServiceError::InvalidCompanyType {
                company_type: company_type.to_string(),
            }),
        }
    }

    /// Parse team role from string
    fn parse_team_role(&self, role: &str) -> Result<TeamRole, CompanyServiceError> {
        match role.to_lowercase().as_str() {
            "admin" => Ok(TeamRole::Admin),
            "write" => Ok(TeamRole::Write),
            "read" => Ok(TeamRole::Read),
            _ => Err(CompanyServiceError::InvalidRole {
                role: role.to_string(),
            }),
        }
    }

    /// Get default permissions for a role
    fn default_permissions_for_role(&self, role: &TeamRole) -> TeamMemberPermissions {
        match role {
            TeamRole::Admin => TeamMemberPermissions {
                can_upload_documents: true,
                can_download_documents: true,
                can_delete_documents: true,
                can_approve_documents: true,
                can_manage_team: true,
                can_view_audit_logs: true,
            },
            TeamRole::Write => TeamMemberPermissions {
                can_upload_documents: true,
                can_download_documents: true,
                can_delete_documents: false,
                can_approve_documents: true,
                can_manage_team: false,
                can_view_audit_logs: false,
            },
            TeamRole::Read => TeamMemberPermissions {
                can_upload_documents: false,
                can_download_documents: true,
                can_delete_documents: false,
                can_approve_documents: false,
                can_manage_team: false,
                can_view_audit_logs: false,
            },
        }
    }

    /// Get or create user by email
    async fn get_or_create_user_by_email(&self, email: &str) -> Result<Uuid, CompanyServiceError> {
        // First try to find existing user
        if let Some(user) = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
            .fetch_optional(&self.db_pool)
            .await?
        {
            return Ok(user.id);
        }

        // Create new user
        let user_id = Uuid::new_v4();
        sqlx::query!(
            r#"
            INSERT INTO users (id, email, user_type, kyc_status, created_at, updated_at)
            VALUES ($1, $2, 'investor', 'not_submitted', NOW(), NOW())
            "#,
            user_id,
            email
        )
        .execute(&self.db_pool)
        .await?;

        Ok(user_id)
    }

    /// Get team member
    async fn get_team_member(
        &self,
        user_id: Uuid,
        company_id: Uuid,
    ) -> Result<TeamMemberResponseDto, CompanyServiceError> {
        let member = sqlx::query(
            r#"
            SELECT tm.*, i.email
            FROM company_memberships tm
            JOIN users i ON tm.user_id = i.id
            WHERE tm.user_id = $1 AND tm.company_id = $2
            "#,
        )
        .bind(user_id)
        .bind(company_id)
        .fetch_optional(&self.db_pool)
        .await?
        .ok_or(CompanyServiceError::TeamMemberNotFound {
            user_id,
            company_id,
        })?;

        self.row_to_team_member_dto(member).await
    }

    /// Create team member helper
    async fn create_team_member(
        &self,
        company_id: Uuid,
        user_id: Uuid,
        role: TeamRole,
        can_upload_documents: bool,
        can_download_documents: bool,
        can_delete_documents: bool,
        can_approve_documents: bool,
        can_manage_team: bool,
        can_view_audit_logs: bool,
        invited_by: Option<Uuid>,
    ) -> Result<Uuid, CompanyServiceError> {
        let team_member_id = Uuid::new_v4();

        sqlx::query!(
            r#"
            INSERT INTO company_memberships (
                id, user_id, company_id, role, status,
                can_upload_documents, can_download_documents, can_delete_documents,
                can_approve_documents, can_manage_team, can_view_audit_logs,
                invited_by, invited_at, accepted_at
            ) VALUES (
                $1, $2, $3, $4, 'active',
                $5, $6, $7, $8, $9, $10, $11, NOW(), NOW()
            )
            "#,
            team_member_id,
            user_id,
            company_id,
            role.to_string(),
            can_upload_documents,
            can_download_documents,
            can_delete_documents,
            can_approve_documents,
            can_manage_team,
            can_view_audit_logs,
            invited_by
        )
        .execute(&self.db_pool)
        .await?;

        Ok(team_member_id)
    }

    /// Convert database row to company DTO
    async fn row_to_company_dto(
        &self,
        row: sqlx::postgres::PgRow,
        team_member_count: i64,
    ) -> CompanyResponseDto {
        CompanyResponseDto {
            id: row.get("id"),
            company_name: row.get("company_name"),
            company_type: row.get("company_type"),
            description: row.get("description"),
            industry: row.get("industry"),
            jurisdiction: row.get("jurisdiction"),
            registration_number: row.get("registration_number"),
            business_email: row.get("business_email"),
            business_phone: row.get("business_phone"),
            kyb_status: row.get("kyb_status"),
            kyb_submitted_at: row.get("kyb_submitted_at"),
            kyb_verified_at: row.get("kyb_verified_at"),
            document_retention_days: row.get::<i32, _>("document_retention_days") as u32,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            team_member_count,
        }
    }

    /// Convert database row to team member DTO
    async fn row_to_team_member_dto(
        &self,
        row: sqlx::postgres::PgRow,
    ) -> Result<TeamMemberResponseDto, CompanyServiceError> {
        Ok(TeamMemberResponseDto {
            id: row.get("id"),
            user_id: row.get("user_id"),
            company_id: row.get("company_id"),
            email: row.get("email"),
            role: row.get("role"),
            status: row.get("status"),
            can_upload_documents: row.get("can_upload_documents"),
            can_download_documents: row.get("can_download_documents"),
            can_delete_documents: row.get("can_delete_documents"),
            can_approve_documents: row.get("can_approve_documents"),
            can_manage_team: row.get("can_manage_team"),
            can_view_audit_logs: row.get("can_view_audit_logs"),
            invited_at: row.get("invited_at"),
            joined_at: row.get("joined_at"),
            last_active_at: row.get("last_active_at"),
            invited_by: row.get("invited_by"),
        })
    }
}
