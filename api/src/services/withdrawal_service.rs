use crate::{BalanceVerificationService, KycValidationService};
use arda_credit_db::{
    create_withdrawal_request, get_pending_withdrawal_requests, get_withdrawal_request_by_id,
    get_withdrawal_requests_by_user_address, update_withdrawal_request_confirmations,
    update_withdrawal_request_state, update_withdrawal_request_submitted,
    update_withdrawal_request_validation, WithdrawalRequest,
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

        // Get current available balance for logging - using simple verification approach
        let available_balance = match self.balance_service
            .verify_balance_for_withdrawal_by_address(&request.user_address, Decimal::ZERO)
            .await
        {
            Ok(result) => result.available_balance,
            Err(_) => Decimal::ZERO,
        };

        // Create initial withdrawal request
        let withdrawal_request = create_withdrawal_request(
            &self.pool,
            user_id,
            &request.user_address,
            &request.recipient_address,
            amount_wei,
            &kyc_status,
            available_balance,
        )
        .await?;

        info!(
            request_id = withdrawal_request.id,
            "Created withdrawal request"
        );

        // Perform validation
        let validation_result = self
            .validate_withdrawal_request(withdrawal_request.id)
            .await?;

        Ok(InitiateWithdrawalResponse {
            request_id: withdrawal_request.id,
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
    ) -> Result<WithdrawalRequest, WithdrawalServiceError> {
        info!("Validating withdrawal request");

        let request = get_withdrawal_request_by_id(&self.pool, request_id).await?;

        // Perform KYC validation
        let kyc_result = self
            .kyc_service
            .validate_kyc_for_withdrawal(request.user_id)
            .await?;

        // Perform balance validation
        let balance_result = self
            .balance_service
            .verify_balance_for_withdrawal(request.user_id, request.amount)
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

        // Update the request with validation results
        let updated_request = update_withdrawal_request_validation(
            &self.pool,
            request_id,
            kyc_result.is_valid,
            balance_result.is_sufficient,
            validation_errors_json,
        )
        .await?;

        if kyc_result.is_valid && balance_result.is_sufficient {
            info!(
                request_id = request_id,
                "Withdrawal request validated successfully"
            );
        } else {
            warn!(
                request_id = request_id,
                kyc_valid = kyc_result.is_valid,
                balance_valid = balance_result.is_sufficient,
                "Withdrawal request validation failed"
            );
        }

        Ok(updated_request)
    }

    /// Get withdrawal request status
    pub async fn get_withdrawal_status(
        &self,
        request_id: i32,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let request = get_withdrawal_request_by_id(&self.pool, request_id).await?;

        Ok(WithdrawalStatusResponse {
            request_id: request.id,
            user_address: request.user_address,
            recipient_address: request.recipient_address,
            amount_usdc: request.amount_usdc.unwrap_or_default(),
            state: request.state,
            kyc_validated: request.kyc_validated,
            balance_validated: request.balance_validated,
            transaction_hash: request.transaction_hash,
            confirmation_count: request.confirmation_count,
            required_confirmations: request.required_confirmations,
            error_message: request.error_message,
            created_at: request.created_at,
            validated_at: request.validated_at,
            submitted_at: request.submitted_at,
            confirmed_at: request.confirmed_at,
        })
    }

    /// Get withdrawal history for a user by wallet address
    pub async fn get_user_withdrawal_history(
        &self,
        user_address: &str,
    ) -> Result<Vec<WithdrawalStatusResponse>, WithdrawalServiceError> {
        let requests = get_withdrawal_requests_by_user_address(&self.pool, user_address).await?;

        let responses = requests
            .into_iter()
            .map(|req| WithdrawalStatusResponse {
                request_id: req.id,
                user_address: req.user_address,
                recipient_address: req.recipient_address,
                amount_usdc: req.amount_usdc.unwrap_or_default(),
                state: req.state,
                kyc_validated: req.kyc_validated,
                balance_validated: req.balance_validated,
                transaction_hash: req.transaction_hash,
                confirmation_count: req.confirmation_count,
                required_confirmations: req.required_confirmations,
                error_message: req.error_message,
                created_at: req.created_at,
                validated_at: req.validated_at,
                submitted_at: req.submitted_at,
                confirmed_at: req.confirmed_at,
            })
            .collect();

        Ok(responses)
    }

    /// Cancel a withdrawal request (only if not yet submitted)
    pub async fn cancel_withdrawal(
        &self,
        request_id: i32,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let request = get_withdrawal_request_by_id(&self.pool, request_id).await?;

        // Can only cancel if not yet submitted
        if matches!(request.state.as_str(), "submitted" | "confirmed") {
            return Err(WithdrawalServiceError::InvalidAmount(
                "Cannot cancel withdrawal that has been submitted to blockchain".to_string(),
            ));
        }

        let updated_request = update_withdrawal_request_state(
            &self.pool,
            request_id,
            "cancelled",
            Some("Cancelled by user"),
        )
        .await?;

        info!(request_id = request_id, "Withdrawal request cancelled");

        Ok(WithdrawalStatusResponse {
            request_id: updated_request.id,
            user_address: updated_request.user_address,
            recipient_address: updated_request.recipient_address,
            amount_usdc: updated_request.amount_usdc.unwrap_or_default(),
            state: updated_request.state,
            kyc_validated: updated_request.kyc_validated,
            balance_validated: updated_request.balance_validated,
            transaction_hash: updated_request.transaction_hash,
            confirmation_count: updated_request.confirmation_count,
            required_confirmations: updated_request.required_confirmations,
            error_message: updated_request.error_message,
            created_at: updated_request.created_at,
            validated_at: updated_request.validated_at,
            submitted_at: updated_request.submitted_at,
            confirmed_at: updated_request.confirmed_at,
        })
    }

    /// Mark withdrawal as submitted to smart contract
    /// This would be called by the smart contract integration service
    pub async fn mark_withdrawal_submitted(
        &self,
        request_id: i32,
        transaction_hash: String,
        block_number: Option<i64>,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let updated_request = update_withdrawal_request_submitted(
            &self.pool,
            request_id,
            &transaction_hash,
            block_number,
        )
        .await?;

        info!(
            request_id = request_id,
            transaction_hash = transaction_hash,
            "Withdrawal request submitted to smart contract"
        );

        Ok(WithdrawalStatusResponse {
            request_id: updated_request.id,
            user_address: updated_request.user_address,
            recipient_address: updated_request.recipient_address,
            amount_usdc: updated_request.amount_usdc.unwrap_or_default(),
            state: updated_request.state,
            kyc_validated: updated_request.kyc_validated,
            balance_validated: updated_request.balance_validated,
            transaction_hash: updated_request.transaction_hash,
            confirmation_count: updated_request.confirmation_count,
            required_confirmations: updated_request.required_confirmations,
            error_message: updated_request.error_message,
            created_at: updated_request.created_at,
            validated_at: updated_request.validated_at,
            submitted_at: updated_request.submitted_at,
            confirmed_at: updated_request.confirmed_at,
        })
    }

    /// Update confirmation count for a withdrawal
    /// This would be called by the blockchain monitoring service
    pub async fn update_withdrawal_confirmations(
        &self,
        request_id: i32,
        confirmation_count: i32,
    ) -> Result<WithdrawalStatusResponse, WithdrawalServiceError> {
        let updated_request =
            update_withdrawal_request_confirmations(&self.pool, request_id, confirmation_count)
                .await?;

        if confirmation_count >= 12 {
            info!(
                request_id = request_id,
                confirmations = confirmation_count,
                "Withdrawal request confirmed"
            );
        }

        Ok(WithdrawalStatusResponse {
            request_id: updated_request.id,
            user_address: updated_request.user_address,
            recipient_address: updated_request.recipient_address,
            amount_usdc: updated_request.amount_usdc.unwrap_or_default(),
            state: updated_request.state,
            kyc_validated: updated_request.kyc_validated,
            balance_validated: updated_request.balance_validated,
            transaction_hash: updated_request.transaction_hash,
            confirmation_count: updated_request.confirmation_count,
            required_confirmations: updated_request.required_confirmations,
            error_message: updated_request.error_message,
            created_at: updated_request.created_at,
            validated_at: updated_request.validated_at,
            submitted_at: updated_request.submitted_at,
            confirmed_at: updated_request.confirmed_at,
        })
    }

    /// Get all pending withdrawal requests that need processing
    pub async fn get_pending_withdrawals(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<WithdrawalStatusResponse>, WithdrawalServiceError> {
        let requests = get_pending_withdrawal_requests(&self.pool, limit).await?;

        let responses = requests
            .into_iter()
            .map(|req| WithdrawalStatusResponse {
                request_id: req.id,
                user_address: req.user_address,
                recipient_address: req.recipient_address,
                amount_usdc: req.amount_usdc.unwrap_or_default(),
                state: req.state,
                kyc_validated: req.kyc_validated,
                balance_validated: req.balance_validated,
                transaction_hash: req.transaction_hash,
                confirmation_count: req.confirmation_count,
                required_confirmations: req.required_confirmations,
                error_message: req.error_message,
                created_at: req.created_at,
                validated_at: req.validated_at,
                submitted_at: req.submitted_at,
                confirmed_at: req.confirmed_at,
            })
            .collect();

        Ok(responses)
    }
}
