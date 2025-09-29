//! Company Invitation Integration Tests
//!
//! Comprehensive tests for the company invitation and onboarding system.
//! Tests the complete flow from invitation creation to acceptance.

use arda_credit_api::{
    company_invitation_service::{
        CompanyInvitationService, CreateInvitationRequest, InvitationError,
    },
    test_utils::{create_test_company, create_test_identity, setup_test_pool},
};
use chrono::Utc;
use serial_test::serial;
use sqlx::PgPool;
use uuid::Uuid;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

async fn create_test_invitation_scenario(pool: &PgPool) -> Result<(Uuid, Uuid, Uuid), sqlx::Error> {
    // Create company owner
    let owner_id = create_test_identity(pool, "owner@example.com", "investor").await?;

    // Create company
    let company_id = create_test_company(pool, "Test Company", "investor", owner_id).await?;

    // Add owner to company as admin
    sqlx::query!(
        r#"
        INSERT INTO team_members (
            company_id, user_id, role, status, can_manage_team,
            invited_by, invited_at, joined_at
        ) VALUES ($1, $2, 'admin', 'active', true, $2, NOW(), NOW())
        "#,
        company_id,
        owner_id
    )
    .execute(pool)
    .await?;

    Ok((company_id, owner_id, Uuid::new_v4()))
}

// ============================================================================
// INVITATION CREATION TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn test_create_invitation_success() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    let request = CreateInvitationRequest {
        email: "newuser@example.com".to_string(),
        role: "write".to_string(),
        personal_message: Some("Welcome to our team!".to_string()),
        custom_role_title: Some("Developer".to_string()),
        expires_in_days: Some(7),
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Validate invitation details
    assert_eq!(invitation.company_id, company_id);
    assert_eq!(invitation.email, "newuser@example.com");
    assert_eq!(invitation.role, "write");
    assert_eq!(invitation.invited_by, inviter_id);
    assert_eq!(invitation.status, "pending");
    assert!(invitation.expires_at > Utc::now());
    assert_eq!(
        invitation.personal_message,
        Some("Welcome to our team!".to_string())
    );
    assert_eq!(invitation.custom_role_title, Some("Developer".to_string()));

    // Validate permissions for 'write' role
    assert!(invitation.permissions.can_upload_documents);
    assert!(invitation.permissions.can_download_documents);
    assert!(!invitation.permissions.can_delete_documents);
    assert!(!invitation.permissions.can_approve_documents);
    assert!(!invitation.permissions.can_manage_team);
    assert!(!invitation.permissions.can_view_audit_logs);

    // Validate invite code format (8 characters, alphanumeric, uppercase)
    assert_eq!(invitation.invite_code.len(), 8);
    assert!(invitation
        .invite_code
        .chars()
        .all(|c| c.is_alphanumeric() && c.is_uppercase()));
}

#[tokio::test]
#[serial]
async fn test_create_invitation_permission_denied() {
    let pool = setup_test_pool().await;
    let (company_id, _, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    // Create a user who is not a team member
    let non_member_id = create_test_identity(&pool, "nonmember@example.com", "investor")
        .await
        .expect("Failed to create test identity");

    let service = CompanyInvitationService::new(pool.clone());

    let request = CreateInvitationRequest {
        email: "newuser@example.com".to_string(),
        role: "read".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: None,
        custom_permissions: None,
    };

    let result = service
        .create_invitation(company_id, non_member_id, request)
        .await;

    assert!(matches!(result, Err(InvitationError::PermissionDenied(_))));
}

#[tokio::test]
#[serial]
async fn test_create_invitation_invalid_role() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    let request = CreateInvitationRequest {
        email: "newuser@example.com".to_string(),
        role: "invalid_role".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: None,
        custom_permissions: None,
    };

    let result = service
        .create_invitation(company_id, inviter_id, request)
        .await;

    assert!(matches!(result, Err(InvitationError::InvalidRole(_))));
}

#[tokio::test]
#[serial]
async fn test_create_invitation_duplicate_pending() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    let request = CreateInvitationRequest {
        email: "duplicate@example.com".to_string(),
        role: "read".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: None,
        custom_permissions: None,
    };

    // Create first invitation
    service
        .create_invitation(company_id, inviter_id, request.clone())
        .await
        .expect("Failed to create first invitation");

    // Try to create duplicate invitation
    let result = service
        .create_invitation(company_id, inviter_id, request)
        .await;

    // Should fail due to unique constraint on pending invitations
    assert!(result.is_err());
}

// ============================================================================
// INVITATION RETRIEVAL TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn test_get_invitation_by_code() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create invitation
    let request = CreateInvitationRequest {
        email: "lookup@example.com".to_string(),
        role: "admin".to_string(),
        personal_message: Some("Join our team!".to_string()),
        custom_role_title: None,
        expires_in_days: Some(14),
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Retrieve invitation by code
    let details = service
        .get_invitation_by_code(&invitation.invite_code)
        .await
        .expect("Failed to get invitation by code");

    // Validate details
    assert_eq!(details.invitation.id, invitation.id);
    assert_eq!(details.invitation.email, "lookup@example.com");
    assert_eq!(details.invitation.role, "admin");
    assert_eq!(details.company_name, "Test Company");
    assert_eq!(
        details.permission_summary,
        "Full access to company resources and team management"
    );

    // Validate admin permissions
    assert!(details.invitation.permissions.can_upload_documents);
    assert!(details.invitation.permissions.can_download_documents);
    assert!(details.invitation.permissions.can_delete_documents);
    assert!(details.invitation.permissions.can_approve_documents);
    assert!(details.invitation.permissions.can_manage_team);
    assert!(details.invitation.permissions.can_view_audit_logs);
}

#[tokio::test]
#[serial]
async fn test_get_invitation_not_found() {
    let pool = setup_test_pool().await;
    let service = CompanyInvitationService::new(pool);

    let result = service.get_invitation_by_code("NOTFOUND").await;

    assert!(matches!(result, Err(InvitationError::NotFound(_))));
}

// ============================================================================
// INVITATION ACCEPTANCE TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn test_accept_invitation_success() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create invitation
    let request = CreateInvitationRequest {
        email: "acceptme@example.com".to_string(),
        role: "write".to_string(),
        personal_message: None,
        custom_role_title: Some("Content Creator".to_string()),
        expires_in_days: None,
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Create user with matching email
    let user_id = create_test_identity(&pool, "acceptme@example.com", "investor")
        .await
        .expect("Failed to create test identity");

    // Accept invitation
    let team_member_id = service
        .accept_invitation(&invitation.invite_code, user_id)
        .await
        .expect("Failed to accept invitation");

    // Validate team membership was created
    let team_member = sqlx::query!(
        r#"
        SELECT tm.id, tm.company_id, tm.user_id, tm.role, tm.status,
               tm.can_upload_documents, tm.can_download_documents, tm.can_delete_documents,
               tm.can_approve_documents, tm.can_manage_team, tm.can_view_audit_logs,
               tm.joined_at
        FROM team_members tm
        WHERE tm.id = $1
        "#,
        team_member_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch team member");

    assert_eq!(team_member.company_id, company_id);
    assert_eq!(team_member.user_id, user_id);
    assert_eq!(team_member.role, "write");
    assert_eq!(team_member.status, "active");
    assert!(team_member.joined_at.is_some());

    // Validate write role permissions
    assert!(team_member.can_upload_documents);
    assert!(team_member.can_download_documents);
    assert!(!team_member.can_delete_documents);
    assert!(!team_member.can_approve_documents);
    assert!(!team_member.can_manage_team);
    assert!(!team_member.can_view_audit_logs);

    // Validate invitation was marked as accepted
    let updated_invitation = sqlx::query!(
        r#"
        SELECT status as "status: String", accepted_at, invited_user_id
        FROM company_invitations
        WHERE id = $1
        "#,
        invitation.id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch updated invitation");

    assert_eq!(updated_invitation.status, "accepted");
    assert!(updated_invitation.accepted_at.is_some());
    assert_eq!(updated_invitation.invited_user_id, Some(user_id));
}

#[tokio::test]
#[serial]
async fn test_accept_invitation_email_mismatch() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create invitation
    let request = CreateInvitationRequest {
        email: "original@example.com".to_string(),
        role: "read".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: None,
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Create user with different email
    let user_id = create_test_identity(&pool, "different@example.com", "investor")
        .await
        .expect("Failed to create test identity");

    // Try to accept invitation
    let result = service
        .accept_invitation(&invitation.invite_code, user_id)
        .await;

    assert!(matches!(result, Err(InvitationError::ValidationError(_))));
}

#[tokio::test]
#[serial]
async fn test_accept_invitation_already_accepted() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create invitation
    let request = CreateInvitationRequest {
        email: "alreadyaccepted@example.com".to_string(),
        role: "read".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: None,
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Create user
    let user_id = create_test_identity(&pool, "alreadyaccepted@example.com", "investor")
        .await
        .expect("Failed to create test identity");

    // Accept invitation first time
    service
        .accept_invitation(&invitation.invite_code, user_id)
        .await
        .expect("Failed to accept invitation");

    // Try to accept again
    let result = service
        .accept_invitation(&invitation.invite_code, user_id)
        .await;

    assert!(matches!(result, Err(InvitationError::AlreadyAccepted)));
}

// ============================================================================
// INVITATION MANAGEMENT TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn test_get_company_invitations() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create multiple invitations
    for i in 1..=3 {
        let request = CreateInvitationRequest {
            email: format!("user{}@example.com", i),
            role: if i % 2 == 0 { "admin" } else { "read" }.to_string(),
            personal_message: Some(format!("Welcome user {}", i)),
            custom_role_title: None,
            expires_in_days: Some(7),
            custom_permissions: None,
        };

        service
            .create_invitation(company_id, inviter_id, request)
            .await
            .expect(&format!("Failed to create invitation {}", i));
    }

    // Get all invitations
    let response = service
        .get_company_invitations(company_id, None, Some(10), Some(0))
        .await
        .expect("Failed to get company invitations");

    assert_eq!(response.invitations.len(), 3);
    assert_eq!(response.total_count, 3);
    assert_eq!(response.pending_count, 3);
    assert_eq!(response.accepted_count, 0);

    // Test filtering by status
    let pending_response = service
        .get_company_invitations(company_id, Some("pending".to_string()), None, None)
        .await
        .expect("Failed to get pending invitations");

    assert_eq!(pending_response.invitations.len(), 3);
}

#[tokio::test]
#[serial]
async fn test_revoke_invitation() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create invitation
    let request = CreateInvitationRequest {
        email: "revokeme@example.com".to_string(),
        role: "write".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: None,
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Revoke invitation
    service
        .revoke_invitation(&invitation.invite_code, inviter_id)
        .await
        .expect("Failed to revoke invitation");

    // Validate invitation is revoked
    let details = service
        .get_invitation_by_code(&invitation.invite_code)
        .await
        .expect("Failed to get invitation details");

    assert_eq!(details.invitation.status, "revoked");
}

// ============================================================================
// ROLE PERMISSIONS TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn test_role_permissions_inheritance() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Test all role types
    let test_cases = vec![
        ("admin", true, true, true, true, true, true),
        ("write", true, true, false, false, false, false),
        ("read", false, true, false, false, false, false),
    ];

    for (role, exp_upload, exp_download, exp_delete, exp_approve, exp_manage, exp_audit) in
        test_cases
    {
        let request = CreateInvitationRequest {
            email: format!("{}@example.com", role),
            role: role.to_string(),
            personal_message: None,
            custom_role_title: None,
            expires_in_days: None,
            custom_permissions: None,
        };

        let invitation = service
            .create_invitation(company_id, inviter_id, request)
            .await
            .expect(&format!("Failed to create invitation for role {}", role));

        // Validate permissions
        assert_eq!(
            invitation.permissions.can_upload_documents, exp_upload,
            "Upload permission mismatch for role {}",
            role
        );
        assert_eq!(
            invitation.permissions.can_download_documents, exp_download,
            "Download permission mismatch for role {}",
            role
        );
        assert_eq!(
            invitation.permissions.can_delete_documents, exp_delete,
            "Delete permission mismatch for role {}",
            role
        );
        assert_eq!(
            invitation.permissions.can_approve_documents, exp_approve,
            "Approve permission mismatch for role {}",
            role
        );
        assert_eq!(
            invitation.permissions.can_manage_team, exp_manage,
            "Manage permission mismatch for role {}",
            role
        );
        assert_eq!(
            invitation.permissions.can_view_audit_logs, exp_audit,
            "Audit permission mismatch for role {}",
            role
        );
    }
}

// ============================================================================
// EXPIRATION TESTS
// ============================================================================

#[tokio::test]
#[serial]
async fn test_expire_old_invitations() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Create an invitation and manually set it to expired
    let request = CreateInvitationRequest {
        email: "expired@example.com".to_string(),
        role: "read".to_string(),
        personal_message: None,
        custom_role_title: None,
        expires_in_days: Some(1),
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    // Manually update expiration to past
    sqlx::query!(
        "UPDATE company_invitations SET expires_at = NOW() - INTERVAL '1 day' WHERE id = $1",
        invitation.id
    )
    .execute(&pool)
    .await
    .expect("Failed to update expiration");

    // Run expiration cleanup
    let expired_count = service
        .expire_old_invitations()
        .await
        .expect("Failed to expire old invitations");

    assert_eq!(expired_count, 1);

    // Validate invitation is marked as expired
    let updated_invitation = sqlx::query!(
        r#"SELECT status as "status: String" FROM company_invitations WHERE id = $1"#,
        invitation.id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch updated invitation");

    assert_eq!(updated_invitation.status, "expired");
}

// ============================================================================
// COMPLETE FLOW INTEGRATION TEST
// ============================================================================

#[tokio::test]
#[serial]
async fn test_complete_invitation_flow() {
    let pool = setup_test_pool().await;
    let (company_id, inviter_id, _) = create_test_invitation_scenario(&pool)
        .await
        .expect("Failed to create test scenario");

    let service = CompanyInvitationService::new(pool.clone());

    // Step 1: Create invitation
    let request = CreateInvitationRequest {
        email: "fullflow@example.com".to_string(),
        role: "write".to_string(),
        personal_message: Some("Welcome to the team!".to_string()),
        custom_role_title: Some("Full Stack Developer".to_string()),
        expires_in_days: Some(14),
        custom_permissions: None,
    };

    let invitation = service
        .create_invitation(company_id, inviter_id, request)
        .await
        .expect("Failed to create invitation");

    assert_eq!(invitation.status, "pending");

    // Step 2: Retrieve invitation by code
    let details = service
        .get_invitation_by_code(&invitation.invite_code)
        .await
        .expect("Failed to get invitation by code");

    assert_eq!(details.invitation.email, "fullflow@example.com");
    assert_eq!(details.invitation.role, "write");
    assert_eq!(details.company_name, "Test Company");

    // Step 3: Create user and accept invitation
    let user_id = create_test_identity(&pool, "fullflow@example.com", "investor")
        .await
        .expect("Failed to create test identity");

    let team_member_id = service
        .accept_invitation(&invitation.invite_code, user_id)
        .await
        .expect("Failed to accept invitation");

    // Step 4: Validate final state
    // Check team membership
    let team_member = sqlx::query!(
        "SELECT role, status FROM team_members WHERE id = $1",
        team_member_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch team member");

    assert_eq!(team_member.role, "write");
    assert_eq!(team_member.status, "active");

    // Check invitation status
    let final_invitation = sqlx::query!(
        r#"SELECT status as "status: String", invited_user_id FROM company_invitations WHERE id = $1"#,
        invitation.id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch final invitation");

    assert_eq!(final_invitation.status, "accepted");
    assert_eq!(final_invitation.invited_user_id, Some(user_id));

    // Step 5: Validate user is now part of company
    let company_memberships = sqlx::query!(
        "SELECT COUNT(*) as count FROM team_members WHERE user_id = $1 AND status = 'active'",
        user_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count user memberships");

    assert_eq!(company_memberships.count, Some(1));
}
