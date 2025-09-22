use arda_credit_db::{get_identity_by_id, DbResult};
use sqlx::PgPool;
use uuid::Uuid;
/// KYC validation service for withdrawal operations
#[derive(Debug, Clone)]
pub struct KycValidationService {
    pool: PgPool,
}
/// KYC validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KycValidationResult {
    pub is_valid: bool,
    pub user_id: Uuid,
    pub kyc_status: String,
    pub reason: Option<String>,
}
impl KycValidationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    /// Validate if user is KYC approved for withdrawal operations
    /// Currently always returns true since KYC is not fully implemented
    pub async fn validate_kyc_for_withdrawal(
        &self,
        user_id: Uuid,
    ) -> DbResult<KycValidationResult> {
        let identity_opt = get_identity_by_id(&self.pool, user_id).await?;
        let identity = match identity_opt {
            Some(identity) => identity,
            None => {
                return Ok(KycValidationResult {
                    is_valid: false,
                    user_id,
                    kyc_status: "not_found".to_string(),
                    reason: Some("User not found".to_string()),
                })
            }
        };

        // TODO: Remove this when KYC system is fully implemented
        // For now, always approve KYC validation for withdrawals
        Ok(KycValidationResult {
            is_valid: true,
            user_id,
            kyc_status: identity.kyc_status.clone(),
            reason: None,
        })
    }
    /// Validate KYC status for a user by wallet address
    /// Returns None if no user is found linked to this address
    pub async fn validate_kyc_for_withdrawal_by_address(
        &self,
        wallet_address: &str,
    ) -> DbResult<Option<KycValidationResult>> {
        // First try to find user by wallet address
        let user_id_result =
            arda_credit_db::get_user_id_by_wallet_address(&self.pool, wallet_address).await;
        match user_id_result {
            Ok(Some(user_id)) => {
                let result = self.validate_kyc_for_withdrawal(user_id).await?;
                Ok(Some(result))
            }
            Ok(None) => {
                // No user linked to this address
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
    /// Check if user exists and has valid KYC
    pub async fn is_user_kyc_approved(&self, user_id: Uuid) -> DbResult<bool> {
        let result = self.validate_kyc_for_withdrawal(user_id).await?;
        Ok(result.is_valid)
    }
    /// Get user's KYC status
    pub async fn get_kyc_status(&self, user_id: Uuid) -> DbResult<String> {
        let identity_opt = get_identity_by_id(&self.pool, user_id).await?;
        match identity_opt {
            Some(identity) => Ok(identity.kyc_status),
            None => Ok("not_found".to_string()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use arda_credit_db::{init_db, register_or_reregister_user, submit_kyc_data};
    use chrono::Utc;
    #[tokio::test]
    async fn test_kyc_validation_service() -> sqlx::Result<()> {
        let pool = init_db().await?;
        let service = KycValidationService::new(pool.clone());

        // Create a test user with unique email
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("kyc-test-{timestamp}@example.com");

        let user_result = register_or_reregister_user(
            &pool,
            "investor",
            &email,
            "hashed_password", // password_hash
        )
        .await
        .unwrap();
        let user_id = match user_result {
            arda_credit_db::RegisterResult::Created(identity) => identity.id,
            arda_credit_db::RegisterResult::Reregistered(identity) => identity.id,
            arda_credit_db::RegisterResult::AlreadyExists => panic!("User should not exist"),
        };
        // Test KYC validation with not_submitted status
        // Note: Currently always returns true (TODO implementation)
        let result = service.validate_kyc_for_withdrawal(user_id).await.unwrap();
        assert!(result.is_valid);
        assert_eq!(result.kyc_status, "not_submitted");
        assert!(result.reason.is_none());
        // Approve KYC
        submit_kyc_data(
            &pool,
            user_id,
            "passport", // document_type
            "hash123",  // document_hash
            Some("US"), // jurisdiction
            Some("AI"), // kyc_investor_type
        )
        .await
        .unwrap();

        // Test KYC validation with approved status
        let result = service.validate_kyc_for_withdrawal(user_id).await.unwrap();
        assert!(result.is_valid);
        assert_eq!(result.kyc_status, "approved");
        assert!(result.reason.is_none());

        Ok(())
    }
}
