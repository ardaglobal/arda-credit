use arda_credit_db::{
    get_identity_by_id, get_user_balance_by_address, get_user_balance_by_id,
    get_user_deposit_balance, DbResult, UserDepositBalance,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

/// Balance verification service for withdrawal operations
#[derive(Debug, Clone)]
pub struct BalanceVerificationService {
    pool: PgPool,
}

/// Balance verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceVerificationResult {
    pub is_sufficient: bool,
    pub available_balance: Decimal,
    pub requested_amount: Decimal,
    pub shortfall: Option<Decimal>,
    pub reason: Option<String>,
}

/// Available balance for withdrawals
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AvailableBalance {
    pub user_address: String,
    #[schema(value_type = String, example = "5000000")]
    pub available_balance: Decimal,
    #[schema(value_type = String, example = "5.000000")]
    pub available_balance_usdc: Decimal,
    #[schema(value_type = String, example = "5000000")]
    pub total_deposited: Decimal,
    #[schema(value_type = String, example = "5.000000")]
    pub total_deposited_usdc: Decimal,
    pub deposit_count: i32,
}

impl BalanceVerificationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Verify if user has sufficient balance for withdrawal by user ID
    pub async fn verify_balance_for_withdrawal(
        &self,
        user_id: Uuid,
        withdrawal_amount: Decimal,
    ) -> DbResult<BalanceVerificationResult> {
        // Get user's available balance
        let balance_result = get_user_balance_by_id(&self.pool, user_id).await;

        match balance_result {
            Ok(Some(available_balance)) => {
                self.create_balance_verification_result(available_balance, withdrawal_amount)
            }
            Ok(None) => {
                // User has no deposit balance
                Ok(BalanceVerificationResult {
                    is_sufficient: false,
                    available_balance: Decimal::ZERO,
                    requested_amount: withdrawal_amount,
                    shortfall: Some(withdrawal_amount),
                    reason: Some("No deposit balance found for user".to_string()),
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Verify if user has sufficient balance for withdrawal by wallet address
    pub async fn verify_balance_for_withdrawal_by_address(
        &self,
        wallet_address: &str,
        withdrawal_amount: Decimal,
    ) -> DbResult<BalanceVerificationResult> {
        // Get user's available balance by address
        let balance_result = get_user_deposit_balance(&self.pool, wallet_address).await;

        match balance_result {
            Ok(Some(balance)) => {
                let available_balance = balance.total_deposited;
                self.create_balance_verification_result(available_balance, withdrawal_amount)
            }
            Ok(None) => {
                // User has no deposit history
                Ok(BalanceVerificationResult {
                    is_sufficient: false,
                    available_balance: Decimal::ZERO,
                    requested_amount: withdrawal_amount,
                    shortfall: Some(withdrawal_amount),
                    reason: Some("No deposit balance found for wallet address".to_string()),
                })
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Get available balance for withdrawals by wallet address
    pub async fn get_available_balance(
        &self,
        wallet_address: &str,
    ) -> DbResult<Option<AvailableBalance>> {
        let balance_result = get_user_deposit_balance(&self.pool, wallet_address).await;

        match balance_result {
            Ok(Some(balance)) => Ok(Some(AvailableBalance {
                user_address: balance.user_address.clone(),
                available_balance: balance.total_deposited,
                available_balance_usdc: balance.total_deposited_usdc.unwrap_or(Decimal::ZERO),
                total_deposited: balance.total_deposited,
                total_deposited_usdc: balance.total_deposited_usdc.unwrap_or(Decimal::ZERO),
                deposit_count: balance.deposit_count,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get available balance for withdrawals by user ID
    pub async fn get_available_balance_by_user_id(
        &self,
        user_id: Uuid,
    ) -> DbResult<Option<AvailableBalance>> {
        // For user ID lookups, we need to get the deposit balance via wallet address
        // First get the user's wallet address
        let user_opt = get_identity_by_id(&self.pool, user_id).await?;

        match user_opt {
            Some(user) => {
                if let Some(wallet_address) = user.wallet_address {
                    self.get_available_balance(&wallet_address).await
                } else {
                    Ok(None) // User has no linked wallet
                }
            }
            None => Ok(None), // User not found
        }
    }

    /// Check if user has sufficient balance
    pub async fn has_sufficient_balance(
        &self,
        user_id: Uuid,
        withdrawal_amount: Decimal,
    ) -> DbResult<bool> {
        let result = self
            .verify_balance_for_withdrawal(user_id, withdrawal_amount)
            .await?;
        Ok(result.is_sufficient)
    }

    /// Check if user has sufficient balance by wallet address
    pub async fn has_sufficient_balance_by_address(
        &self,
        wallet_address: &str,
        withdrawal_amount: Decimal,
    ) -> DbResult<bool> {
        let result = self
            .verify_balance_for_withdrawal_by_address(wallet_address, withdrawal_amount)
            .await?;
        Ok(result.is_sufficient)
    }

    /// Helper to create balance verification result
    fn create_balance_verification_result(
        &self,
        available_balance: Decimal,
        withdrawal_amount: Decimal,
    ) -> DbResult<BalanceVerificationResult> {
        let is_sufficient = available_balance >= withdrawal_amount;
        let shortfall = if is_sufficient {
            None
        } else {
            Some(withdrawal_amount - available_balance)
        };
        let reason = if !is_sufficient {
            Some(format!(
                "Insufficient balance. Available: {}, Requested: {}, Shortfall: {}",
                available_balance,
                withdrawal_amount,
                shortfall.unwrap_or_default()
            ))
        } else {
            None
        };

        Ok(BalanceVerificationResult {
            is_sufficient,
            available_balance,
            requested_amount: withdrawal_amount,
            shortfall,
            reason,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arda_credit_db::{create_usdc_deposit, init_db, register_or_reregister_user};
    use chrono::Utc;

    #[tokio::test]
    async fn test_balance_verification_service() -> sqlx::Result<()> {
        let pool = init_db().await?;
        let service = BalanceVerificationService::new(pool.clone());
        let wallet_address = "0x1234567890123456789012345678901234567890";

        // Create a test user with linked wallet
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("balance-test-{timestamp}@example.com");
        let unique_wallet = format!("0x{:040x}", timestamp as u64);

        let user_result = register_or_reregister_user(
            &pool,
            "investor",
            &email,
            "hashed_password", // password_hash
        )
        .await?;

        let user_id = match user_result {
            arda_credit_db::RegisterResult::Created(identity) => identity.id,
            arda_credit_db::RegisterResult::Reregistered(identity) => identity.id,
            arda_credit_db::RegisterResult::AlreadyExists => panic!("User should not exist"),
        };

        // Link wallet address
        sqlx::query("UPDATE identities SET wallet_address = $1 WHERE id = $2")
            .bind(&unique_wallet)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        // Test balance verification with no deposits
        let result = service
            .verify_balance_for_withdrawal_by_address(&unique_wallet, Decimal::new(1000000, 0))
            .await
            .unwrap();
        assert!(!result.is_sufficient);
        assert_eq!(result.available_balance, Decimal::ZERO);

        // Create a deposit
        let deposit_amount = Decimal::new(5000000, 0); // 5 USDC in wei
        let tx_hash = format!("0x{:064x}", timestamp);
        let event_tx_hash = format!("0x{:064x}", timestamp + 1);
        create_usdc_deposit(
            &pool,
            &unique_wallet,
            deposit_amount,
            &tx_hash,
            &event_tx_hash, // event_transaction_hash
            Utc::now().timestamp(),
            0,
            (timestamp % 1000) as i64, // unique log_index
        )
        .await
        .unwrap();

        // Update deposit status to confirmed to trigger balance update
        sqlx::query("UPDATE usdc_deposits SET status = 'confirmed' WHERE transaction_hash = $1")
            .bind(&tx_hash)
            .execute(&pool)
            .await
            .unwrap();

        // Test balance verification with sufficient balance
        let withdrawal_amount = Decimal::new(3000000, 0); // 3 USDC
        let result = service
            .verify_balance_for_withdrawal_by_address(&unique_wallet, withdrawal_amount)
            .await
            .unwrap();
        assert!(result.is_sufficient);
        assert_eq!(result.available_balance, deposit_amount);

        // Test balance verification with insufficient balance
        let large_withdrawal = Decimal::new(10000000, 0); // 10 USDC
        let result = service
            .verify_balance_for_withdrawal_by_address(&unique_wallet, large_withdrawal)
            .await
            .unwrap();
        assert!(!result.is_sufficient);
        assert!(result.shortfall.is_some());
        assert_eq!(result.shortfall.unwrap(), large_withdrawal - deposit_amount);

        Ok(())
    }
}
