use arda_credit_db::{get_account_by_address, get_available_balance, DbResult};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
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

impl BalanceVerificationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Verify if account has sufficient balance for withdrawal by account ID
    pub async fn verify_balance_for_withdrawal(
        &self,
        account_id: Uuid,
        withdrawal_amount: Decimal,
        currency: &str,
    ) -> DbResult<BalanceVerificationResult> {
        // Get account's available balance
        let available_balance = get_available_balance(&self.pool, account_id, currency).await?;

        self.create_balance_verification_result(available_balance, withdrawal_amount)
    }

    /// Verify if account has sufficient balance for withdrawal by wallet address
    /// This is a convenience method that looks up account first
    pub async fn verify_balance_for_withdrawal_by_address(
        &self,
        wallet_address: &str,
        withdrawal_amount: Decimal,
        currency: &str,
    ) -> DbResult<BalanceVerificationResult> {
        // Look up account by address
        let account = get_account_by_address(&self.pool, wallet_address).await?;

        match account {
            Some(acc) => {
                let available_balance =
                    get_available_balance(&self.pool, acc.account_id, currency).await?;
                self.create_balance_verification_result(available_balance, withdrawal_amount)
            }
            None => {
                // No account found for this address
                Ok(BalanceVerificationResult {
                    is_sufficient: false,
                    available_balance: Decimal::ZERO,
                    requested_amount: withdrawal_amount,
                    shortfall: Some(withdrawal_amount),
                    reason: Some(format!("No account found for address {}", wallet_address)),
                })
            }
        }
    }

    /// Check if account has sufficient balance (shorthand)
    pub async fn has_sufficient_balance(
        &self,
        account_id: Uuid,
        withdrawal_amount: Decimal,
        currency: &str,
    ) -> DbResult<bool> {
        let result = self
            .verify_balance_for_withdrawal(account_id, withdrawal_amount, currency)
            .await?;
        Ok(result.is_sufficient)
    }

    /// Check if account has sufficient balance by address (shorthand)
    pub async fn has_sufficient_balance_by_address(
        &self,
        wallet_address: &str,
        withdrawal_amount: Decimal,
        currency: &str,
    ) -> DbResult<bool> {
        let result = self
            .verify_balance_for_withdrawal_by_address(wallet_address, withdrawal_amount, currency)
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
    use arda_credit_db::{
        create_deposit, get_account_by_address, init_db, register_or_reregister_user,
        update_deposit_status,
    };
    use chrono::Utc;

    #[tokio::test]
    async fn test_balance_verification_service() -> sqlx::Result<()> {
        let pool = init_db().await?;
        let service = BalanceVerificationService::new(pool.clone());

        // Create a test user with linked wallet
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("balance-test-{timestamp}@example.com");
        let unique_wallet = format!("0x{:040x}", timestamp as u64);

        let user_result = register_or_reregister_user(&pool, "investor", &email).await?;

        let user_id = match user_result {
            arda_credit_db::RegisterResult::Created(identity) => identity.id,
            arda_credit_db::RegisterResult::Reregistered(identity) => identity.id,
            arda_credit_db::RegisterResult::AlreadyExists => panic!("User should not exist"),
        };

        // Link wallet address (triggers account auto-creation)
        sqlx::query("UPDATE identities SET wallet_address = $1 WHERE id = $2")
            .bind(&unique_wallet)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();

        // Test balance verification with no deposits
        let result = service
            .verify_balance_for_withdrawal_by_address(
                &unique_wallet,
                Decimal::new(1000000, 0),
                "USDC",
            )
            .await
            .unwrap();
        assert!(!result.is_sufficient);
        assert_eq!(result.available_balance, Decimal::ZERO);

        // Create a deposit (will auto-find account)
        let deposit_amount = Decimal::new(5000000, 0); // 5 USDC in wei
        let tx_hash = format!("0x{:064x}", timestamp);
        let event_tx_hash = format!("0x{:064x}", timestamp + 1);
        let deposit = create_deposit(
            &pool,
            &unique_wallet,
            deposit_amount,
            "USDC",
            &tx_hash,
            &event_tx_hash,
            12345678,
            Utc::now().timestamp(),
            (timestamp % 1000) as i64,
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
        )
        .await
        .unwrap();

        // Update deposit status to confirmed to trigger balance update
        update_deposit_status(&pool, deposit.id, "confirmed", 12, Some(Utc::now()))
            .await
            .unwrap();

        // Test balance verification with sufficient balance
        let withdrawal_amount = Decimal::new(3000000, 0); // 3 USDC
        let result = service
            .verify_balance_for_withdrawal_by_address(&unique_wallet, withdrawal_amount, "USDC")
            .await
            .unwrap();
        assert!(result.is_sufficient);
        assert_eq!(result.available_balance, deposit_amount);

        // Test balance verification with insufficient balance
        let large_withdrawal = Decimal::new(10000000, 0); // 10 USDC
        let result = service
            .verify_balance_for_withdrawal_by_address(&unique_wallet, large_withdrawal, "USDC")
            .await
            .unwrap();
        assert!(!result.is_sufficient);
        assert!(result.shortfall.is_some());
        assert_eq!(result.shortfall.unwrap(), large_withdrawal - deposit_amount);

        Ok(())
    }
}
