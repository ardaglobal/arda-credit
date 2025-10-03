use crate::{BalanceVerificationService, KycValidationService};
use arda_credit_db::{
    create_withdrawal_request, get_account_by_address, get_pending_withdrawals,
    get_withdrawal_by_id, get_withdrawals_by_account_id, mark_withdrawal_submitted,
    update_withdrawal_confirmations, update_withdrawal_state, update_withdrawal_validation,
    Withdrawal,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{error, info, instrument, warn};
use utoipa::ToSchema;

/// Withdrawal transaction management service
#[derive(Debug, Clone)]
pub struct WithdrawalTransactionService {
    pool: PgPool,
    kyc_service: KycValidationService,
    balance_service: BalanceVerificationService,
}

/// Request to initiate a withdrawal
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InitiateWithdrawalRequest {
    pub user_address: String,
    pub recipient_address: String,
    #[schema(value_type = String, example = "100.50")]
    pub amount_usdc: Decimal, // Human-readable USDC amount (e.g., 100.50)
}

/// Response when withdrawal is initiated
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct InitiateWithdrawalResponse {
    pub request_id: i32,
    pub state: String,
    pub kyc_validated: bool,
    pub balance_validated: bool,
    #[schema(value_type = String, example = "100.50")]
    pub amount_usdc: Decimal,
    pub error_message: Option<String>,
    pub validation_errors: Option<serde_json::Value>,
}

/// Withdrawal request status response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WithdrawalStatusResponse {
    pub request_id: i32,
    pub user_address: String,
    pub recipient_address: String,
    #[schema(value_type = String, example = "100.50")]
    pub amount_usdc: Decimal,
    pub state: String,
    pub kyc_validated: bool,
    pub balance_validated: bool,
    pub transaction_hash: Option<String>,
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub error_message: Option<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub validated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub submitted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub confirmed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, thiserror::Error)]
pub enum WithdrawalServiceError {
    #[error("Database error: {0}")]
    Database(#[from] arda_credit_db::DbError),
    #[error("SQL error: {0}")]
    Sql(#[from] sqlx::Error),
    #[error("User not found for address: {0}")]
    UserNotFound(String),
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),
    #[error("KYC validation failed: {0}")]
    KycValidationFailed(String),
    #[error("Insufficient balance: {0}")]
    InsufficientBalance(String),
}

impl WithdrawalTransactionService {
    pub fn new(pool: PgPool) -> Self {
        let kyc_service = KycValidationService::new(pool.clone());
        let balance_service = BalanceVerificationService::new(pool.clone());

        Self {
            pool,
            kyc_service,
            balance_service,
        }
    }

    /// Initiate a new withdrawal request with full validation
    #[instrument(skip(self), fields(user_address = %request.user_address, amount = %request.amount_usdc))]
    pub async fn initiate_withdrawal(
        &self,
        request: InitiateWithdrawalRequest,
    ) -> Result<InitiateWithdrawalResponse, WithdrawalServiceError> {
        info!("Initiating withdrawal request");

        // Validate amount
        if request.amount_usdc <= Decimal::ZERO {
            return Err(WithdrawalServiceError::InvalidAmount(
                "Amount must be positive".to_string(),
            ));
        }

        // Convert USDC to wei (6 decimals)
        let amount_wei = request.amount_usdc * Decimal::new(1_000_000, 0);

        // Find user by wallet address
        let user_id =
            match arda_credit_db::get_user_id_by_wallet_address(&self.pool, &request.user_address)
                .await
            {
                Ok(Some(id)) => id,
                Ok(None) => {
                    return Err(WithdrawalServiceError::UserNotFound(request.user_address));
                }
                Err(e) => return Err(WithdrawalServiceError::Database(e.into())),
            };

        // Get current KYC status for logging
        let kyc_status = self
            .kyc_service
            .get_kyc_status(user_id)
            .await
            .unwrap_or_else(|_| "unknown".to_string());

        // Get current available balance for logging
        let available_balance = match self
            .balance_service
            .verify_balance_for_withdrawal_by_address(&request.user_address, Decimal::ZERO, "USDC")
            .await
        {
            Ok(result) => result.available_balance,
            Err(_) => Decimal::ZERO,
        };

        // Create initial withdrawal request (will lookup account_id by address)
        let withdrawal = create_withdrawal_request(
            &self.pool,
            &request.user_address,
            &request.recipient_address,
            amount_wei,
            "USDC",
            user_id,
            &kyc_status,
            available_balance,
        )
        .await?;

        info!(request_id = withdrawal.id, "Created withdrawal request");

        // Perform validation
        let validation_result = self.validate_withdrawal_request(withdrawal.id).await?;

        Ok(InitiateWithdrawalResponse {
            request_id: withdrawal.id,
            state: validation_result.state.clone(),
            kyc_validated: validation_result.kyc_validated,
            balance_validated: validation_result.balance_validated,
            amount_usdc: request.amount_usdc,
            error_message: validation_result.error_message.clone(),
            validation_errors: validation_result.validation_errors.clone(),
        })
    }

    /// Validate a withdrawal request (KYC + balance checks)
    #[instrument(skip(self), fields(request_id = %request_id))]
    pub async fn validate_withdrawal_request(
        &self,
        request_id: i32,
    ) -> Result<Withdrawal, WithdrawalServiceError> {
        info!("Validating withdrawal request");

        let withdrawal = get_withdrawal_by_id(&self.pool, request_id).await?;

        // Perform KYC validation (if user_id is available)
        let kyc_result = if let Some(user_id) = withdrawal.requested_by_user_id {
            self.kyc_service
                .validate_kyc_for_withdrawal(user_id)
                .await?
        } else {
            // For contract-initiated withdrawals, assume KYC valid
            arda_credit_db::KycValidationResult {
                is_valid: true,
                user_id: uuid::Uuid::nil(),
                kyc_status: "N/A".to_string(),
                reason: None,
            }
        };

        // Perform balance validation
        let account_id = withdrawal.account_id.ok_or_else(|| {
            WithdrawalServiceError::Database(arda_credit_db::DbError::InvalidState(
                "Withdrawal missing account_id".to_string(),
            ))
        })?;

        let balance_result = self
            .balance_service
            .verify_balance_for_withdrawal(account_id, withdrawal.amount, &withdrawal.currency)
            .await?;

        // Create validation errors if any
        let mut validation_errors = serde_json::Map::new();

        if !kyc_result.is_valid {
            validation_errors.insert(
                "kyc".to_string(),
                serde_json::json!({
                    "valid": false,
                    "reason": kyc_result.reason.unwrap_or_default()
                }),
            );
        } else {
            validation_errors.insert(
                "kyc".to_string(),
                serde_json::json!({
                    "valid": true,
                    "status": kyc_result.kyc_status
                }),
            );
        }

        if !balance_result.is_sufficient {
            validation_errors.insert(
                "balance".to_string(),
                serde_json::json!({
                    "valid": false,
                    "reason": balance_result.reason.unwrap_or_default(),
                    "available": balance_result.available_balance,
                    "requested": balance_result.requested_amount,
                    "shortfall": balance_result.shortfall
                }),
            );
        } else {
            validation_errors.insert(
                "balance".to_string(),
                serde_json::json!({
                    "valid": true,
                    "available": balance_result.available_balance,
                    "requested": balance_result.requested_amount
                }),
            );
        }

        let validation_errors_json = if validation_errors.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(validation_errors))
        };

        // Update withdrawal with validation results
        let updated_withdrawal = update_withdrawal_validation(
            &self.pool,
            request_id,
            kyc_result.is_valid,
            balance_result.is_sufficient,
            validation_errors_json,
        )
        .await?;

        if kyc_result.is_valid && balance_result.is_sufficient {
            info!(request_id = request_id, "Withdrawal validated successfully");
        } else {
            warn!(
                request_id = request_id,
                kyc_valid = kyc_result.is_valid,
                balance_valid = balance_result.is_sufficient,
                "Withdrawal validation failed"
            );
        }

        Ok(updated_withdrawal)
    }

    /// Get withdrawal status
    pub async fn get_withdrawal_status(
        &self,
        request_id: i32,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let withdrawal = get_withdrawal_by_id(&self.pool, request_id).await?;

        // Convert amount from wei to USDC
        let amount_usdc = withdrawal.amount / Decimal::new(1_000_000, 0);

        Ok(WithdrawalStatusResponse {
            request_id: withdrawal.id,
            user_address: withdrawal.address.clone(),
            recipient_address: withdrawal.recipient_address.clone(),
            amount_usdc,
            state: withdrawal.state,
            kyc_validated: withdrawal.kyc_validated,
            balance_validated: withdrawal.balance_validated,
            transaction_hash: withdrawal.transaction_hash,
            confirmation_count: withdrawal.confirmation_count,
            required_confirmations: withdrawal.required_confirmations,
            error_message: withdrawal.error_message,
            created_at: withdrawal.created_at,
            validated_at: withdrawal.validated_at,
            submitted_at: withdrawal.submitted_at,
            confirmed_at: withdrawal.confirmed_at,
        })
    }

    /// Get withdrawal history for an account by address
    pub async fn get_user_withdrawal_history(
        &self,
        user_address: &str,
    ) -> Result<Vec<WithdrawalStatusResponse>, WithdrawalServiceError> {
        // Look up account by address
        let account = get_account_by_address(&self.pool, user_address)
            .await?
            .ok_or_else(|| WithdrawalServiceError::UserNotFound(user_address.to_string()))?;

        let withdrawals =
            get_withdrawals_by_account_id(&self.pool, account.account_id, None).await?;

        let responses = withdrawals
            .into_iter()
            .map(|w| {
                let amount_usdc = w.amount / Decimal::new(1_000_000, 0);
                WithdrawalStatusResponse {
                    request_id: w.id,
                    user_address: w.address,
                    recipient_address: w.recipient_address,
                    amount_usdc,
                    state: w.state,
                    kyc_validated: w.kyc_validated,
                    balance_validated: w.balance_validated,
                    transaction_hash: w.transaction_hash,
                    confirmation_count: w.confirmation_count,
                    required_confirmations: w.required_confirmations,
                    error_message: w.error_message,
                    created_at: w.created_at,
                    validated_at: w.validated_at,
                    submitted_at: w.submitted_at,
                    confirmed_at: w.confirmed_at,
                }
            })
            .collect();

        Ok(responses)
    }

    /// Cancel a withdrawal (only if not yet submitted)
    pub async fn cancel_withdrawal(
        &self,
        request_id: i32,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let withdrawal = get_withdrawal_by_id(&self.pool, request_id).await?;

        // Can only cancel if not yet submitted
        if matches!(withdrawal.state.as_str(), "submitted" | "confirmed") {
            return Err(WithdrawalServiceError::InvalidAmount(
                "Cannot cancel withdrawal that has been submitted to blockchain".to_string(),
            ));
        }

        let updated = update_withdrawal_state(
            &self.pool,
            request_id,
            "cancelled",
            Some("Cancelled by user"),
        )
        .await?;

        info!(request_id = request_id, "Withdrawal cancelled");

        let amount_usdc = updated.amount / Decimal::new(1_000_000, 0);

        Ok(WithdrawalStatusResponse {
            request_id: updated.id,
            user_address: updated.address,
            recipient_address: updated.recipient_address,
            amount_usdc,
            state: updated.state,
            kyc_validated: updated.kyc_validated,
            balance_validated: updated.balance_validated,
            transaction_hash: updated.transaction_hash,
            confirmation_count: updated.confirmation_count,
            required_confirmations: updated.required_confirmations,
            error_message: updated.error_message,
            created_at: updated.created_at,
            validated_at: updated.validated_at,
            submitted_at: updated.submitted_at,
            confirmed_at: updated.confirmed_at,
        })
    }

    /// Mark withdrawal as submitted to smart contract
    pub async fn mark_withdrawal_submitted(
        &self,
        request_id: i32,
        transaction_hash: String,
        block_number: Option<i64>,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let updated =
            mark_withdrawal_submitted(&self.pool, request_id, &transaction_hash, block_number)
                .await?;

        info!(
            request_id = request_id,
            transaction_hash = transaction_hash,
            "Withdrawal submitted to smart contract"
        );

        let amount_usdc = updated.amount / Decimal::new(1_000_000, 0);

        Ok(WithdrawalStatusResponse {
            request_id: updated.id,
            user_address: updated.address,
            recipient_address: updated.recipient_address,
            amount_usdc,
            state: updated.state,
            kyc_validated: updated.kyc_validated,
            balance_validated: updated.balance_validated,
            transaction_hash: updated.transaction_hash,
            confirmation_count: updated.confirmation_count,
            required_confirmations: updated.required_confirmations,
            error_message: updated.error_message,
            created_at: updated.created_at,
            validated_at: updated.validated_at,
            submitted_at: updated.submitted_at,
            confirmed_at: updated.confirmed_at,
        })
    }

    /// Update confirmation count for a withdrawal
    pub async fn update_withdrawal_confirmations(
        &self,
        request_id: i32,
        confirmation_count: i32,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let updated =
            update_withdrawal_confirmations(&self.pool, request_id, confirmation_count).await?;

        if confirmation_count >= 12 {
            info!(
                request_id = request_id,
                confirmations = confirmation_count,
                "Withdrawal confirmed"
            );
        }

        let amount_usdc = updated.amount / Decimal::new(1_000_000, 0);

        Ok(WithdrawalStatusResponse {
            request_id: updated.id,
            user_address: updated.address,
            recipient_address: updated.recipient_address,
            amount_usdc,
            state: updated.state,
            kyc_validated: updated.kyc_validated,
            balance_validated: updated.balance_validated,
            transaction_hash: updated.transaction_hash,
            confirmation_count: updated.confirmation_count,
            required_confirmations: updated.required_confirmations,
            error_message: updated.error_message,
            created_at: updated.created_at,
            validated_at: updated.validated_at,
            submitted_at: updated.submitted_at,
            confirmed_at: updated.confirmed_at,
        })
    }

    /// Get all pending withdrawals that need processing
    pub async fn get_pending_withdrawals(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<WithdrawalStatusResponse>, WithdrawalServiceError> {
        let withdrawals = get_pending_withdrawals(&self.pool, None, limit).await?;

        let responses = withdrawals
            .into_iter()
            .map(|w| {
                let amount_usdc = w.amount / Decimal::new(1_000_000, 0);
                WithdrawalStatusResponse {
                    request_id: w.id,
                    user_address: w.address,
                    recipient_address: w.recipient_address,
                    amount_usdc,
                    state: w.state,
                    kyc_validated: w.kyc_validated,
                    balance_validated: w.balance_validated,
                    transaction_hash: w.transaction_hash,
                    confirmation_count: w.confirmation_count,
                    required_confirmations: w.required_confirmations,
                    error_message: w.error_message,
                    created_at: w.created_at,
                    validated_at: w.validated_at,
                    submitted_at: w.submitted_at,
                    confirmed_at: w.confirmed_at,
                }
            })
            .collect();

        Ok(responses)
    }
}
