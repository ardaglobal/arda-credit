//! Core loan processing logic for zkVM execution
//!
//! This module contains the core state transition logic that will run inside
//! the SP1 zkVM. It processes batches of mixed transaction types while maintaining
//! privacy guarantees.

#[cfg(test)]
use crate::balance::BalanceUpdateType;
use crate::balance::{BalanceError, BalanceState, BalanceUpdate};
use crate::credit_line::{CreditLineError, CreditLineState};
use crate::loan::LoanTerms;
use crate::loan::{Loan, LoanError};
use crate::merchant::{MerchantError, MerchantState};
use crate::pool::{Pool, PoolConfig, PoolState};
use crate::transactions::{LoanBatch, LoanTransaction, TransactionType};
use crate::user::{User, UserError, UserState, UserType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during loan processing
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    #[error("User error: {0}")]
    UserError(#[from] UserError),

    #[error("Loan error: {0}")]
    LoanError(#[from] LoanError),

    #[error("Balance error: {0}")]
    BalanceError(#[from] BalanceError),

    #[error("Merchant error: {0}")]
    MerchantError(#[from] MerchantError),

    #[error("Credit line error: {0}")]
    CreditLineError(#[from] CreditLineError),

    #[error("Invalid transaction type for operation")]
    InvalidTransactionType,

    #[error("Invalid timestamp: {timestamp} is outside acceptable range (must be between {min_timestamp} and {max_timestamp})")]
    InvalidTimestamp {
        timestamp: u64,
        min_timestamp: u64,
        max_timestamp: u64,
    },

    #[error("Transaction timestamp {transaction_timestamp} cannot be older than current state timestamp {state_timestamp}")]
    TimestampGoesBackward {
        transaction_timestamp: u64,
        state_timestamp: u64,
    },

    #[error("User not found")]
    UserNotFound,

    #[error("Invalid transaction data")]
    InvalidTransactionData,
}

/// Global state for loan processing (maintains user, loan, and pool state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLoanState {
    /// All users in the system
    pub users: HashMap<[u8; 32], User>,

    /// All loans in the system
    pub loans: HashMap<[u8; 32], Loan>,

    /// All liquidity pools in the system
    pub pools: PoolState,

    /// Merchant state tracking
    pub merchant_state: MerchantState,

    /// Credit line state tracking
    pub credit_line_state: CreditLineState,

    /// User balance tracking state
    pub balance_state: BalanceState,

    /// Configurable loan terms
    pub loan_terms: LoanTerms,

    /// Transaction counter for ordering
    pub transaction_count: u64,

    /// Last update timestamp
    pub last_updated: u64,
}

impl Default for GlobalLoanState {
    fn default() -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
            pools: PoolState::new(),
            merchant_state: MerchantState::new(),
            credit_line_state: CreditLineState::new(),
            balance_state: BalanceState::new(),
            loan_terms: LoanTerms::default(),
            transaction_count: 0,
            last_updated: 0,
        }
    }
}

impl GlobalLoanState {
    /// Create a new empty loan state with default loan terms
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new loan state with custom loan terms
    #[must_use]
    pub fn with_loan_terms(loan_terms: LoanTerms) -> Self {
        Self {
            users: HashMap::new(),
            loans: HashMap::new(),
            pools: PoolState::new(),
            merchant_state: MerchantState::new(),
            balance_state: BalanceState::new(),
            credit_line_state: CreditLineState::new(),
            loan_terms,
            transaction_count: 0,
            last_updated: 0,
        }
    }

    /// Update loan terms configuration
    pub const fn set_loan_terms(&mut self, loan_terms: LoanTerms) {
        self.loan_terms = loan_terms;
    }

    /// Process a single transaction and update state
    ///
    /// # Errors
    /// Returns `ProcessingError` for validation failures or transaction processing errors
    pub fn process_transaction(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), ProcessingError> {
        // Validate transaction with current state context
        validate_transaction_with_state(transaction, self)?;

        match transaction.transaction_type {
            TransactionType::UserRegistration => {
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_user_registration(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::KycSubmission => {
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_kyc_submission(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::KycVerification => {
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_kyc_verification(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::LoanOrigination => {
                let mut loan_state = crate::loan::LoanState {
                    loans: self.loans.clone(),
                };
                loan_state.process_loan_origination(transaction, &self.users, &self.loan_terms)?;
                self.loans = loan_state.loans;

                // Create balance updates for loan origination
                let metadata = transaction.metadata.as_bytes();
                let mut borrower_id = [0u8; 32];
                let mut lender_id = [0u8; 32];
                borrower_id.copy_from_slice(&metadata[0..32]);
                lender_id.copy_from_slice(&metadata[32..64]);

                let balance_updates = self.balance_state.create_origination_updates(
                    transaction.transaction_id,
                    transaction.loan_id,
                    borrower_id,
                    lender_id,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance updates to state
                for update in balance_updates {
                    self.balance_state.apply_update(update)?;
                }
            }
            TransactionType::LoanPayment => {
                // Get loan info before processing payment for balance updates
                let loan =
                    self.loans
                        .get(&transaction.loan_id)
                        .ok_or(ProcessingError::LoanError(LoanError::LoanNotFound {
                            loan_id: transaction.loan_id,
                        }))?;
                let lender_id = loan.lender_id;
                let payer_id = transaction.user_id;

                let mut loan_state = crate::loan::LoanState {
                    loans: self.loans.clone(),
                };
                loan_state.process_loan_payment(transaction, &self.users)?;
                self.loans = loan_state.loans;

                // Create balance updates for loan payment
                let balance_updates = self.balance_state.create_payment_updates(
                    transaction.transaction_id,
                    transaction.loan_id,
                    payer_id,
                    lender_id,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance updates to state
                for update in balance_updates {
                    self.balance_state.apply_update(update)?;
                }
            }
            TransactionType::LoanCloseout => {
                let mut loan_state = crate::loan::LoanState {
                    loans: self.loans.clone(),
                };
                loan_state.process_loan_closeout(transaction, &self.users)?;
                self.loans = loan_state.loans;
            }
            TransactionType::UserDeposit => {
                // Process user deposit by creating a balance update
                // For deposits, metadata contains: [wallet_address(32)] + [remaining_32_bytes_unused]
                let metadata = transaction.metadata.as_bytes();
                let mut wallet_address = [0u8; 32];
                wallet_address.copy_from_slice(&metadata[0..32]);

                let balance_update = self.balance_state.create_deposit_update(
                    transaction.transaction_id,
                    transaction.user_id,
                    wallet_address,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance update to state
                self.balance_state.apply_update(balance_update)?;
            }
            TransactionType::UserWithdrawal => {
                // Process user withdrawal by creating a balance update
                // For withdrawals, metadata contains: [recipient_address(32)] + [remaining_32_bytes_unused]
                let metadata = transaction.metadata.as_bytes();
                let mut recipient_address = [0u8; 32];
                recipient_address.copy_from_slice(&metadata[0..32]);

                let balance_update = self.balance_state.create_withdrawal_update(
                    transaction.transaction_id,
                    transaction.user_id,
                    recipient_address,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance update to state
                self.balance_state.apply_update(balance_update)?;
            }
            TransactionType::InvestorRegistration => {
                // Process investor registration - update user type to investor
                if let Some(user) = self.users.get_mut(&transaction.user_id) {
                    user.user_type = crate::user::UserType::Investor;
                } else {
                    return Err(ProcessingError::UserNotFound);
                }
            }
            TransactionType::PoolCreation => {
                // Process pool creation transaction
                // Metadata format: [max_ltv_ratio(8)] + [max_interest_rate(8)] + [repayment_period_days(4)] + [remaining_44_bytes_unused]
                let metadata = transaction.metadata.as_bytes();

                let max_ltv_ratio = u64::from_le_bytes([
                    metadata[0],
                    metadata[1],
                    metadata[2],
                    metadata[3],
                    metadata[4],
                    metadata[5],
                    metadata[6],
                    metadata[7],
                ]);
                let max_interest_rate = u64::from_le_bytes([
                    metadata[8],
                    metadata[9],
                    metadata[10],
                    metadata[11],
                    metadata[12],
                    metadata[13],
                    metadata[14],
                    metadata[15],
                ]);
                let repayment_period_days =
                    u32::from_le_bytes([metadata[16], metadata[17], metadata[18], metadata[19]]);

                let pool_config = PoolConfig {
                    max_ltv_ratio,
                    max_interest_rate,
                    repayment_period_days,
                };

                // Create new pool using loan_id as pool_id
                let pool = Pool::new(
                    transaction.loan_id, // Using loan_id field as pool_id
                    transaction.user_id, // investor_id
                    pool_config,
                    transaction.created_at,
                )
                .map_err(|_| ProcessingError::InvalidTransactionData)?;

                // Add pool to state
                self.pools
                    .add_pool(pool)
                    .map_err(|_| ProcessingError::InvalidTransactionData)?;
            }
            TransactionType::MerchantRegistration => {
                // Process merchant registration in both user and merchant state
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_merchant_registration(transaction)?;
                self.users = user_state.users;

                // Also register in merchant state
                self.merchant_state
                    .process_merchant_registration(transaction)?;
            }
            TransactionType::MerchantProfileUpdate => {
                // Process merchant profile update in merchant state
                self.merchant_state.process_profile_update(transaction)?;
            }
            TransactionType::CreditLineRequest => {
                // Process credit line request from merchants
                // This requires KYC approval and will be used for automatic matching with liquidity pools
                let user =
                    self.users
                        .get(&transaction.user_id)
                        .ok_or(ProcessingError::UserError(UserError::UserNotFound {
                            user_id: transaction.user_id,
                        }))?;

                // Ensure user is a merchant
                if !matches!(user.user_type, UserType::Merchant) {
                    return Err(ProcessingError::UserError(UserError::InvalidUserType {
                        expected: UserType::Merchant,
                        actual: user.user_type,
                    }));
                }

                // Ensure merchant is KYC approved for credit line requests
                if !user.is_kyc_approved() {
                    return Err(ProcessingError::UserError(UserError::UserNotKycApproved {
                        user_id: transaction.user_id,
                    }));
                }

                // Create credit line request
                // Metadata contains: [requested_amount(8)] + [requested_rate_apr(8)] + [requested_tenor_days(4)] + [remaining_44_bytes_unused]
                let metadata = transaction.metadata.as_bytes();
                let requested_amount = u64::from_le_bytes([
                    metadata[0],
                    metadata[1],
                    metadata[2],
                    metadata[3],
                    metadata[4],
                    metadata[5],
                    metadata[6],
                    metadata[7],
                ]);
                let requested_rate_apr = u64::from_le_bytes([
                    metadata[8],
                    metadata[9],
                    metadata[10],
                    metadata[11],
                    metadata[12],
                    metadata[13],
                    metadata[14],
                    metadata[15],
                ]);
                let requested_tenor_days =
                    u32::from_le_bytes([metadata[16], metadata[17], metadata[18], metadata[19]]);

                // Create credit line request
                let request = crate::credit_line::CreditLineRequest::new(
                    transaction.transaction_id,
                    transaction.user_id,
                    requested_amount,
                    if requested_rate_apr > 0 {
                        Some(requested_rate_apr)
                    } else {
                        None
                    },
                    requested_tenor_days,
                    transaction.created_at,
                );

                self.credit_line_state.add_request(request);
            }
            TransactionType::CreditLineApproval => {
                // Process credit line approval
                // Metadata contains: [request_id(32)] + [approved_amount(8)] + [approved_rate(8)] + [pool_id(32)] + [repayment_days(4)] + [remaining_unused]
                let metadata = transaction.metadata.as_bytes();
                let mut request_id = [0u8; 32];
                request_id.copy_from_slice(&metadata[0..32]);

                let approved_amount = u64::from_le_bytes([
                    metadata[32],
                    metadata[33],
                    metadata[34],
                    metadata[35],
                    metadata[36],
                    metadata[37],
                    metadata[38],
                    metadata[39],
                ]);
                let approved_rate = u64::from_le_bytes([
                    metadata[40],
                    metadata[41],
                    metadata[42],
                    metadata[43],
                    metadata[44],
                    metadata[45],
                    metadata[46],
                    metadata[47],
                ]);
                let mut pool_id = [0u8; 32];
                pool_id.copy_from_slice(&metadata[48..80]);
                let repayment_window_days =
                    u32::from_le_bytes([metadata[80], metadata[81], metadata[82], metadata[83]]);

                // Approve the credit line request and create active credit line
                self.credit_line_state.approve_request(
                    request_id,
                    transaction.transaction_id, // Use transaction ID as credit line ID
                    approved_amount,
                    approved_rate,
                    pool_id,
                    repayment_window_days,
                    transaction.created_at,
                )?;
            }
            TransactionType::CreditLineRejection => {
                // Process credit line rejection
                // Metadata contains: [request_id(32)] + [rejection_reason...] (remaining bytes as string)
                let metadata = transaction.metadata.as_bytes();
                let mut request_id = [0u8; 32];
                request_id.copy_from_slice(&metadata[0..32]);

                // Extract rejection reason from remaining metadata
                let reason_bytes = &metadata[32..];
                let reason_end = reason_bytes
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(reason_bytes.len());
                let rejection_reason =
                    String::from_utf8_lossy(&reason_bytes[..reason_end]).into_owned();

                // Reject the credit line request
                self.credit_line_state.reject_request(
                    request_id,
                    rejection_reason,
                    transaction.created_at,
                )?;
            }
            TransactionType::CreditLineDrawdown => {
                // Process credit line drawdown (utilizing available credit)
                // Metadata contains: [credit_line_id(32)] + [recipient_address(32)]
                let metadata = transaction.metadata.as_bytes();
                let mut credit_line_id = [0u8; 32];
                credit_line_id.copy_from_slice(&metadata[0..32]);
                let mut recipient_address = [0u8; 32];
                recipient_address.copy_from_slice(&metadata[32..64]);

                // Process drawdown in credit line state
                self.credit_line_state.process_drawdown(
                    credit_line_id,
                    transaction.amount,
                    recipient_address,
                    transaction.created_at,
                )?;

                // Create corresponding balance update for the drawdown
                let balance_update = self.balance_state.create_credit_drawdown_update(
                    transaction.transaction_id,
                    transaction.user_id,
                    recipient_address,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance update to state
                self.balance_state.apply_update(balance_update)?;
            }
            TransactionType::CreditLineRepayment => {
                // Process credit line repayment
                // Metadata contains: [credit_line_id(32)] + [payment_type(1)] + [remaining_31_bytes_unused]
                let metadata = transaction.metadata.as_bytes();
                let mut credit_line_id = [0u8; 32];
                credit_line_id.copy_from_slice(&metadata[0..32]);
                let payment_type = metadata[32]; // 0 = principal, 1 = interest, 2 = both

                // Process repayment in credit line state
                self.credit_line_state.process_repayment(
                    credit_line_id,
                    transaction.amount,
                    payment_type,
                    transaction.created_at,
                )?;

                // Create corresponding balance update for the repayment
                let balance_update = self.balance_state.create_credit_repayment_update(
                    transaction.transaction_id,
                    transaction.user_id,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance update to state
                self.balance_state.apply_update(balance_update)?;
            }
            TransactionType::SubmitTurnover => {
                // Process turnover submission in merchant state
                self.merchant_state
                    .process_turnover_submission(transaction)?;
            }
            TransactionType::VerifyDocument => {
                // Process document verification in merchant state
                self.merchant_state
                    .process_document_verification(transaction)?;
            }
        }

        // Update global state
        self.transaction_count += 1;
        self.last_updated = transaction.created_at;

        Ok(())
    }

    /// Process a batch of transactions
    ///
    /// # Errors
    /// Returns `ProcessingError` for batch validation failures or transaction processing errors
    pub fn process_batch(&mut self, batch: &LoanBatch) -> Result<(), ProcessingError> {
        // Validate batch timestamp bounds
        validate_timestamp_bounds(batch.created_at)?;

        // Validate temporal ordering within the batch
        validate_batch_temporal_ordering(batch)?;

        for transaction in &batch.transactions {
            self.process_transaction(transaction)?;
        }
        Ok(())
    }

    /// Get user by ID (read-only)
    #[must_use]
    pub fn get_user(&self, user_id: &[u8; 32]) -> Option<&User> {
        self.users.get(user_id)
    }

    /// Get loan by ID (read-only)
    #[must_use]
    pub fn get_loan(&self, loan_id: &[u8; 32]) -> Option<&Loan> {
        self.loans.get(loan_id)
    }

    /// Get current state hash (for Merkle tree)
    #[must_use]
    pub fn compute_state_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let serialized = serde_json::to_string(&self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(serialized.as_bytes());
        hasher.finalize().into()
    }

    /// Compute total portfolio balance across all loans
    /// This aggregates all outstanding loan balances for portfolio visibility
    #[must_use]
    pub fn compute_total_balance(&self) -> u128 {
        let mut total_balance = 0u128;

        for loan in self.loans.values() {
            // Add outstanding balance for each active loan
            if matches!(
                loan.loan_status,
                crate::loan::LoanStatus::Active | crate::loan::LoanStatus::Originated
            ) {
                total_balance = total_balance.saturating_add(u128::from(loan.outstanding_balance));
            }
        }

        total_balance
    }

    /// Get user's current balance
    #[must_use]
    pub fn get_user_balance(&self, user_id: &[u8; 32]) -> u128 {
        self.balance_state.get_balance(user_id)
    }

    /// Get balance update history for a user
    #[must_use]
    pub fn get_user_balance_history(
        &self,
        user_id: &[u8; 32],
        limit: Option<usize>,
    ) -> Vec<&BalanceUpdate> {
        self.balance_state.get_user_update_history(user_id, limit)
    }

    /// Get balance updates for a specific loan
    #[must_use]
    pub fn get_loan_balance_updates(&self, loan_id: &[u8; 32]) -> Vec<&BalanceUpdate> {
        self.balance_state.get_loan_balance_updates(loan_id)
    }

    /// Get total value of all user balances
    #[must_use]
    pub fn compute_total_user_balance(&self) -> u128 {
        self.balance_state.compute_total_balance()
    }
}

/// Process a complete loan batch and return the state transition
///
/// # Errors
/// Returns `ProcessingError` if batch processing fails due to validation errors,
/// temporal ordering issues, or transaction processing errors.
pub fn process_loan_batch(
    initial_state: GlobalLoanState,
    batch: &LoanBatch,
) -> Result<GlobalLoanState, ProcessingError> {
    let mut state = initial_state;
    // Additional validation: ensure batch timestamp is not older than current state
    if batch.created_at < state.last_updated {
        return Err(ProcessingError::TimestampGoesBackward {
            transaction_timestamp: batch.created_at,
            state_timestamp: state.last_updated,
        });
    }

    state.process_batch(batch)?;
    Ok(state)
}

/// Validate a loan transaction before processing
///
/// # Errors
/// Returns `ProcessingError` if the transaction is invalid due to incorrect
/// amount, loan ID requirements, or timestamp bounds.
pub fn validate_transaction(transaction: &LoanTransaction) -> Result<(), ProcessingError> {
    // Basic validation
    match transaction.transaction_type {
        TransactionType::LoanOrigination
        | TransactionType::LoanPayment
        | TransactionType::UserDeposit
        | TransactionType::UserWithdrawal => {
            if transaction.amount == 0 {
                return Err(ProcessingError::InvalidTransactionType);
            }
        }
        _ => {}
    }

    // Validate all zeros loan_id for non-loan transactions
    let zero_loan_id = [0u8; 32];
    match transaction.transaction_type {
        TransactionType::UserRegistration
        | TransactionType::KycSubmission
        | TransactionType::KycVerification
        | TransactionType::UserDeposit
        | TransactionType::UserWithdrawal
        | TransactionType::InvestorRegistration
        | TransactionType::MerchantRegistration
        | TransactionType::MerchantProfileUpdate
        | TransactionType::CreditLineRequest
        | TransactionType::CreditLineApproval
        | TransactionType::CreditLineRejection
        | TransactionType::SubmitTurnover
        | TransactionType::VerifyDocument => {
            if transaction.loan_id != zero_loan_id {
                return Err(ProcessingError::InvalidTransactionType);
            }
        }
        // PoolCreation uses loan_id field as the pool_id, so it can be non-zero
        TransactionType::PoolCreation => {}
        _ => {}
    }

    // Additional validation for timestamp bounds
    validate_timestamp_bounds(transaction.created_at)?;

    Ok(())
}

/// Validate a loan transaction with state context for temporal ordering and KYC requirements
///
/// # Errors
/// Returns `ProcessingError` if the transaction fails basic validation,
/// if the transaction timestamp is older than the current state timestamp,
/// or if KYC requirements are not met.
pub fn validate_transaction_with_state(
    transaction: &LoanTransaction,
    current_state: &GlobalLoanState,
) -> Result<(), ProcessingError> {
    // Run basic validation first
    validate_transaction(transaction)?;

    // Validate temporal ordering - transaction cannot be older than current state
    if transaction.created_at < current_state.last_updated {
        return Err(ProcessingError::TimestampGoesBackward {
            transaction_timestamp: transaction.created_at,
            state_timestamp: current_state.last_updated,
        });
    }

    // Validate KYC requirements for transactions that require it
    if transaction.transaction_type.requires_kyc() {
        let user =
            current_state
                .users
                .get(&transaction.user_id)
                .ok_or(ProcessingError::UserError(
                    crate::user::UserError::UserNotFound {
                        user_id: transaction.user_id,
                    },
                ))?;

        if !user.is_kyc_approved() {
            return Err(ProcessingError::UserError(
                crate::user::UserError::KycNotApproved {
                    user_id: transaction.user_id,
                },
            ));
        }

        // For withdrawals, also validate sufficient balance
        if transaction.transaction_type == TransactionType::UserWithdrawal {
            let current_balance = current_state
                .balance_state
                .get_balance(&transaction.user_id);
            let withdrawal_amount = u128::from(transaction.amount);

            if current_balance < withdrawal_amount {
                return Err(ProcessingError::BalanceError(
                    crate::balance::BalanceError::InsufficientBalance {
                        requested: withdrawal_amount,
                        available: current_balance,
                    },
                ));
            }
        }
    }

    Ok(())
}

/// Validate that a timestamp is within reasonable bounds for production use
fn validate_timestamp_bounds(timestamp: u64) -> Result<(), ProcessingError> {
    // Define reasonable bounds for loan system timestamps
    // Min: January 1, 2020 (1577836800)
    // Max: January 1, 2030 (1893456000) - 10 years from now for reasonable future limit
    const MIN_TIMESTAMP: u64 = 1_577_836_800; // Jan 1, 2020
    const MAX_TIMESTAMP: u64 = 1_893_456_000; // Jan 1, 2030

    if !(MIN_TIMESTAMP..=MAX_TIMESTAMP).contains(&timestamp) {
        return Err(ProcessingError::InvalidTimestamp {
            timestamp,
            min_timestamp: MIN_TIMESTAMP,
            max_timestamp: MAX_TIMESTAMP,
        });
    }

    Ok(())
}

/// Validate temporal ordering within a batch of transactions
fn validate_batch_temporal_ordering(batch: &LoanBatch) -> Result<(), ProcessingError> {
    let mut prev_timestamp = 0u64;

    for transaction in &batch.transactions {
        // Ensure transactions within batch are not going backwards in time
        if transaction.created_at < prev_timestamp {
            return Err(ProcessingError::TimestampGoesBackward {
                transaction_timestamp: transaction.created_at,
                state_timestamp: prev_timestamp,
            });
        }

        // Validate each transaction's timestamp bounds
        validate_timestamp_bounds(transaction.created_at)?;

        prev_timestamp = transaction.created_at;
    }

    Ok(())
}

// Re-export the old LoanState name for backward compatibility
pub type LoanState = GlobalLoanState;

// ============================================================================
// NEW LOAN OPERATIONS INTERFACE
// ============================================================================

/// Operations for loan processing (alternative to `LoanTransaction`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoanOperation {
    /// Create a new loan
    LoanCreation {
        loan_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        principal: u128,
        interest_rate: u64,
        term_months: u32,
    },
    /// Make a payment towards a loan
    Payment {
        loan_id: [u8; 32],
        payer_id: [u8; 32],
        amount: u128,
    },
    /// Fund an originated loan
    LoanFunding {
        loan_id: [u8; 32],
        lender_id: [u8; 32],
    },
    /// Update balance (for reconciliation)
    BalanceUpdate {
        loan_id: [u8; 32],
        new_balance: u128,
    },
}

impl LoanOperation {
    /// Get the transaction type for this operation
    #[must_use]
    pub const fn transaction_type(&self) -> u8 {
        match self {
            Self::LoanCreation { .. } => 1,
            Self::Payment { .. } => 2,
            Self::LoanFunding { .. } => 3,
            Self::BalanceUpdate { .. } => 4,
        }
    }

    /// Convert operation to nullifier hash for Merkle tree
    #[must_use]
    pub fn to_nullifier(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        match self {
            Self::LoanCreation {
                loan_id,
                borrower_id,
                lender_id,
                principal,
                ..
            } => {
                hasher.update(b"loan_creation");
                hasher.update(loan_id);
                hasher.update(borrower_id);
                hasher.update(lender_id);
                hasher.update(principal.to_le_bytes());
            }
            Self::Payment {
                loan_id,
                payer_id,
                amount,
            } => {
                hasher.update(b"payment");
                hasher.update(loan_id);
                hasher.update(payer_id);
                hasher.update(amount.to_le_bytes());
            }
            Self::LoanFunding { loan_id, lender_id } => {
                hasher.update(b"funding");
                hasher.update(loan_id);
                hasher.update(lender_id);
            }
            Self::BalanceUpdate {
                loan_id,
                new_balance,
            } => {
                hasher.update(b"balance_update");
                hasher.update(loan_id);
                hasher.update(new_balance.to_le_bytes());
            }
        }
        hasher.finalize().into()
    }
}

/// Simple Merkle tree implementation for loan operations
#[derive(Debug, Clone)]
pub struct MerkleTree {
    root: [u8; 32],
    leaves: Vec<[u8; 32]>,
}

impl MerkleTree {
    /// Create a new Merkle tree from an existing root
    #[must_use]
    pub const fn from_root(root: [u8; 32]) -> Self {
        Self {
            root,
            leaves: Vec::new(),
        }
    }

    /// Insert a new leaf (nullifier) into the tree
    pub fn insert(&mut self, leaf: [u8; 32]) {
        self.leaves.push(leaf);
        self.root = self.compute_root();
    }

    /// Get the current root hash
    #[must_use]
    pub const fn root(&self) -> [u8; 32] {
        self.root
    }

    /// Compute the Merkle root from all leaves
    fn compute_root(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        if self.leaves.is_empty() {
            return [0u8; 32];
        }
        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(chunk[0]);
                if chunk.len() > 1 {
                    hasher.update(chunk[1]);
                } else {
                    hasher.update(chunk[0]); // Duplicate if odd number
                }
                next_level.push(hasher.finalize().into());
            }

            current_level = next_level;
        }

        current_level[0]
    }
}

/// Process a batch of loan operations and return public state transition
#[must_use]
pub fn process_loan_operations(
    previous_balance: u128,
    operations: &[LoanOperation],
    previous_root: [u8; 32],
    batch_timestamp: u64,
) -> crate::LoanBatchPublicValues {
    use sha2::{Digest, Sha256};

    let mut current_balance = previous_balance;
    let mut merkle_tree = MerkleTree::from_root(previous_root);
    // Compute transaction types hash
    let mut hasher = Sha256::new();
    for operation in operations {
        hasher.update([operation.transaction_type()]);
    }
    let transaction_types_hash: [u8; 32] = hasher.finalize().into();

    // Process operations
    for operation in operations {
        // Update balance based on operation type
        current_balance = apply_operation_to_balance(current_balance, operation);

        // Update Merkle tree with operation commitment
        let nullifier = operation.to_nullifier();
        merkle_tree.insert(nullifier);
    }

    let new_root = merkle_tree.root();

    crate::LoanBatchPublicValues {
        previous_state_root: previous_root.into(),
        new_state_root: new_root.into(),
        transaction_count: u32::try_from(operations.len()).unwrap_or(0),
        batch_timestamp,
        transaction_types_hash: transaction_types_hash.into(),
        previousTotalBalance: previous_balance,
        currentTotalBalance: current_balance,
    }
}

/// Apply a loan operation to the current balance
const fn apply_operation_to_balance(balance: u128, operation: &LoanOperation) -> u128 {
    match operation {
        LoanOperation::LoanCreation { principal, .. } => balance.saturating_add(*principal),
        LoanOperation::Payment { amount, .. } => balance.saturating_add(*amount),
        LoanOperation::LoanFunding { .. } => balance, // No balance change, just state change
        LoanOperation::BalanceUpdate { new_balance, .. } => *new_balance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loan::LoanStatus;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus, UserType};

    fn create_test_user_registration(user_id: [u8; 32], user_type: u8) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1_640_995_200, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&[user_type]),
        }
    }

    fn create_kyc_verification(user_id: [u8; 32], approved: bool) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [2u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1_640_995_300, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&[u8::from(approved)]),
        }
    }

    fn create_loan_origination(
        loan_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&borrower_id);
        metadata[32..64].copy_from_slice(&lender_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount,
            created_at: 1_640_995_400, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    #[test]
    fn test_full_loan_lifecycle() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // 1. Register users
        let borrower_reg = create_test_user_registration(borrower_id, 1); // Investor
        let lender_reg = create_test_user_registration(lender_id, 2); // Lender

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();

        // 2. KYC verification
        let borrower_kyc = create_kyc_verification(borrower_id, true);
        let lender_kyc = create_kyc_verification(lender_id, true);

        state.process_transaction(&borrower_kyc).unwrap();
        state.process_transaction(&lender_kyc).unwrap();

        // Verify users are registered and KYC approved
        let borrower = state.get_user(&borrower_id).unwrap();
        let lender = state.get_user(&lender_id).unwrap();
        assert_eq!(borrower.kyc_status, KycStatus::Approved);
        assert_eq!(lender.kyc_status, KycStatus::Approved);

        // 3. Give lender sufficient balance for loan origination (for testing)
        let initial_funding = BalanceUpdate::new(
            [99u8; 32], // funding transaction ID
            lender_id,
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            100_000,       // Give enough for the 50000 loan
            0,             // previous balance
            1_640_995_100, // before loan origination
            Some("Test funding".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(initial_funding).unwrap();

        // 4. Loan origination
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);
        state.process_transaction(&loan_origination).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Originated);
        assert_eq!(loan.principal_amount, 50000);

        // 5. Loan payment
        let payment = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 25000,             // Partial payment
            created_at: 1_640_995_500, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&payment).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Active); // Now active
        assert_eq!(loan.outstanding_balance, 25000);

        // 5. Final payment
        let final_payment = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 25000,             // Final payment
            created_at: 1_640_995_600, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&final_payment).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::PaidOff);
        assert_eq!(loan.outstanding_balance, 0);

        // 6. Loan closeout
        let closeout = LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: lender_id,
            loan_id,
            transaction_type: TransactionType::LoanCloseout,
            amount: 0,
            created_at: 1_640_995_700, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&closeout).unwrap();

        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Closed);

        // Verify transaction count
        assert_eq!(state.transaction_count, 8); // 2 registrations + 2 KYC verifications + 1 origination + 2 payments + 1 closeout
    }

    #[test]
    fn test_loan_origination_without_kyc_fails() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [10u8; 32];

        // Register users but don't KYC them
        let borrower_reg = create_test_user_registration(borrower_id, 1);
        let lender_reg = create_test_user_registration(lender_id, 2);

        state.process_transaction(&borrower_reg).unwrap();
        state.process_transaction(&lender_reg).unwrap();

        // Try to originate loan without KYC
        let loan_origination = create_loan_origination(loan_id, borrower_id, lender_id, 50000);
        let result = state.process_transaction(&loan_origination);

        assert!(matches!(
            result,
            Err(ProcessingError::UserError(UserError::KycNotApproved { .. }))
                | Err(ProcessingError::LoanError(LoanError::UserError(
                    UserError::KycNotApproved { .. }
                )))
        ));
    }

    #[test]
    fn test_process_batch() {
        let mut state = GlobalLoanState::new();

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];

        let batch = LoanBatch {
            batch_id: [100u8; 32],
            transactions: vec![
                create_test_user_registration(borrower_id, 1),
                create_test_user_registration(lender_id, 2),
                create_kyc_verification(borrower_id, true),
                create_kyc_verification(lender_id, true),
            ],
            previous_state_root: [0u8; 32],
            created_at: 1_640_995_200, // January 1, 2022 - valid timestamp
        };

        let result = state.process_batch(&batch);
        assert!(result.is_ok());

        assert_eq!(state.users.len(), 2);
        assert_eq!(state.transaction_count, 4);

        // Verify both users are KYC approved
        assert!(state.get_user(&borrower_id).unwrap().is_kyc_approved());
        assert!(state.get_user(&lender_id).unwrap().is_kyc_approved());
    }

    #[test]
    fn test_configurable_loan_terms() {
        // Test with custom loan terms
        let custom_terms = LoanTerms::new(1200, 60); // 12% APR, 5 year term
        let mut state = GlobalLoanState::with_loan_terms(custom_terms);

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];
        let loan_id = [3u8; 32];

        // Register and KYC both users
        setup_kycd_users(&mut state, borrower_id, lender_id);

        // Create loan origination transaction
        let mut loan_metadata = MetadataArray::empty();
        loan_metadata.as_mut_bytes()[0..32].copy_from_slice(&borrower_id);
        loan_metadata.as_mut_bytes()[32..64].copy_from_slice(&lender_id);

        let base_timestamp = 1_640_995_200; // January 1, 2022
        let loan_tx = LoanTransaction {
            transaction_id: [10u8; 32],
            user_id: lender_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount: 1_000_000, // $10,000 in cents
            created_at: base_timestamp + 5000,
            metadata: loan_metadata,
        };

        // Give lender sufficient balance for loan origination (for testing)
        let initial_funding = BalanceUpdate::new(
            [98u8; 32], // funding transaction ID
            lender_id,
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            2_000_000,      // Give enough for the 1,000,000 loan
            0,              // previous balance
            base_timestamp, // same timeframe
            Some("Test funding for configurable terms".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(initial_funding).unwrap();

        state.process_transaction(&loan_tx).unwrap();

        // Verify loan uses custom terms
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.interest_rate, 1200); // Custom 12% APR
        assert_eq!(loan.term_months, 60); // Custom 5 year term
        assert_eq!(loan.principal_amount, 1_000_000);
        assert_eq!(loan.outstanding_balance, 1_000_000);

        // Verify maturity date calculation
        let expected_maturity = (base_timestamp + 5000) + (60_u64 * 30 * 24 * 60 * 60); // 60 months
        assert_eq!(loan.maturity_date, expected_maturity);

        // Test with default loan terms
        let mut default_state = GlobalLoanState::new();
        assert_eq!(default_state.loan_terms.interest_rate, 750); // Default 7.5%
        assert_eq!(default_state.loan_terms.term_months, 36); // Default 3 years

        // Test updating loan terms on existing state
        let new_terms = LoanTerms::new(500, 24); // 5% APR, 2 year term
        default_state.set_loan_terms(new_terms);
        assert_eq!(default_state.loan_terms.interest_rate, 500);
        assert_eq!(default_state.loan_terms.term_months, 24);
    }

    /// Helper function to set up KYC'd users for testing
    fn setup_kycd_users(state: &mut GlobalLoanState, borrower_id: [u8; 32], lender_id: [u8; 32]) {
        let base_timestamp = 1_640_995_200; // January 1, 2022

        // Register borrower
        let mut borrower_metadata = MetadataArray::empty();
        borrower_metadata.as_mut_bytes()[0] = UserType::Investor as u8;
        let borrower_reg_tx = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: base_timestamp + 1000,
            metadata: borrower_metadata,
        };
        state.process_transaction(&borrower_reg_tx).unwrap();

        // Register lender
        let mut lender_metadata = MetadataArray::empty();
        lender_metadata.as_mut_bytes()[0] = UserType::Lender as u8;
        let lender_reg_tx = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: lender_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: base_timestamp + 1100,
            metadata: lender_metadata,
        };
        state.process_transaction(&lender_reg_tx).unwrap();

        // Submit KYC for borrower
        let borrower_kyc_submit_tx = LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: base_timestamp + 2000,
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&borrower_kyc_submit_tx).unwrap();

        // Submit KYC for lender
        let lender_kyc_submit_tx = LoanTransaction {
            transaction_id: [7u8; 32],
            user_id: lender_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: base_timestamp + 2100,
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&lender_kyc_submit_tx).unwrap();

        // Verify KYC for borrower
        let mut borrower_verify_metadata = MetadataArray::empty();
        borrower_verify_metadata.as_mut_bytes()[0] = 1; // Approved
        let borrower_kyc_verify_tx = LoanTransaction {
            transaction_id: [8u8; 32],
            user_id: borrower_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: base_timestamp + 3000,
            metadata: borrower_verify_metadata,
        };
        state.process_transaction(&borrower_kyc_verify_tx).unwrap();

        // Verify KYC for lender
        let mut lender_verify_metadata = MetadataArray::empty();
        lender_verify_metadata.as_mut_bytes()[0] = 1; // Approved
        let lender_kyc_verify_tx = LoanTransaction {
            transaction_id: [9u8; 32],
            user_id: lender_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: base_timestamp + 3100,
            metadata: lender_verify_metadata,
        };
        state.process_transaction(&lender_kyc_verify_tx).unwrap();
    }
    #[test]
    fn test_timestamp_validation_bounds() {
        // Test valid timestamp (within bounds)
        let valid_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1_640_995_200, // January 1, 2022 - valid
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction(&valid_transaction).is_ok());

        // Test invalid timestamp (too old)
        let old_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1_000_000_000, // September 9, 2001 - too old
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction(&old_transaction).is_err());

        // Test invalid timestamp (too far in future)
        let future_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 2_000_000_000, // May 18, 2033 - too far in future
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction(&future_transaction).is_err());
    }

    #[test]
    fn test_temporal_ordering_validation() {
        let mut state = GlobalLoanState::new();
        state.last_updated = 2000;

        // Test transaction newer than state (valid)
        let newer_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1_640_995_200, // Valid timestamp after state
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction_with_state(&newer_transaction, &state).is_ok());

        // Test transaction older than state (invalid)
        let older_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: 1000, // Before state timestamp
            metadata: MetadataArray::empty(),
        };
        assert!(validate_transaction_with_state(&older_transaction, &state).is_err());
    }

    #[test]
    fn test_batch_temporal_ordering() {
        // Test valid batch with properly ordered timestamps
        let transactions = vec![
            LoanTransaction {
                transaction_id: [1u8; 32],
                user_id: [1u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1_640_995_200, // Base time
                metadata: MetadataArray::empty(),
            },
            LoanTransaction {
                transaction_id: [2u8; 32],
                user_id: [2u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1_640_995_300, // 100 seconds later
                metadata: MetadataArray::empty(),
            },
        ];

        let valid_batch = LoanBatch {
            batch_id: [1u8; 32],
            transactions,
            previous_state_root: [0u8; 32],
            created_at: 1_640_995_200,
        };
        assert!(validate_batch_temporal_ordering(&valid_batch).is_ok());

        // Test invalid batch with backwards timestamps
        let invalid_transactions = vec![
            LoanTransaction {
                transaction_id: [1u8; 32],
                user_id: [1u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1_640_995_300, // Later time first
                metadata: MetadataArray::empty(),
            },
            LoanTransaction {
                transaction_id: [2u8; 32],
                user_id: [2u8; 32],
                loan_id: [0u8; 32],
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1_640_995_200, // Earlier time second - invalid!
                metadata: MetadataArray::empty(),
            },
        ];

        let invalid_batch = LoanBatch {
            batch_id: [1u8; 32],
            transactions: invalid_transactions,
            previous_state_root: [0u8; 32],
            created_at: 1_640_995_200,
        };
        assert!(validate_batch_temporal_ordering(&invalid_batch).is_err());
    }

    #[test]
    fn test_process_loan_operations() {
        let previous_balance = 100_000u128; // $1000
        let previous_root = [0u8; 32];
        let batch_timestamp = 1_640_995_200;

        let operations = vec![
            LoanOperation::LoanCreation {
                loan_id: [1u8; 32],
                borrower_id: [2u8; 32],
                lender_id: [3u8; 32],
                principal: 50_000,
                interest_rate: 750,
                term_months: 36,
            },
            LoanOperation::Payment {
                loan_id: [1u8; 32],
                payer_id: [2u8; 32],
                amount: 10_000,
            },
        ];

        let public_values = process_loan_operations(
            previous_balance,
            &operations,
            previous_root,
            batch_timestamp,
        );

        // Check that balance was updated correctly
        assert_eq!(public_values.previousTotalBalance, 100_000);
        assert_eq!(public_values.currentTotalBalance, 160_000); // 100k + 50k loan + 10k payment
        assert_eq!(public_values.transaction_count, 2);
        assert_eq!(public_values.batch_timestamp, batch_timestamp);

        // Check that state root changed (merkle tree was updated)
        assert_ne!(
            public_values.previous_state_root,
            public_values.new_state_root
        );

        // Check that transaction types hash is computed
        let zero_hash: alloy_sol_types::private::FixedBytes<32> = [0u8; 32].into();
        assert_ne!(public_values.transaction_types_hash, zero_hash);
    }

    #[test]
    fn test_loan_operation_nullifiers() {
        let operation1 = LoanOperation::LoanCreation {
            loan_id: [1u8; 32],
            borrower_id: [2u8; 32],
            lender_id: [3u8; 32],
            principal: 50_000,
            interest_rate: 750,
            term_months: 36,
        };

        let operation2 = LoanOperation::Payment {
            loan_id: [1u8; 32],
            payer_id: [2u8; 32],
            amount: 10_000,
        };

        let nullifier1 = operation1.to_nullifier();
        let nullifier2 = operation2.to_nullifier();

        // Nullifiers should be different
        assert_ne!(nullifier1, nullifier2);

        // Same operation should produce same nullifier
        let nullifier1_copy = operation1.to_nullifier();
        assert_eq!(nullifier1, nullifier1_copy);
    }

    #[test]
    fn test_merkle_tree_basic() {
        let mut tree = MerkleTree::from_root([0u8; 32]);
        let initial_root = tree.root();

        // Insert a leaf
        tree.insert([1u8; 32]);
        let new_root = tree.root();

        // Root should change after insertion
        assert_ne!(initial_root, new_root);

        // Insert another leaf
        tree.insert([2u8; 32]);
        let final_root = tree.root();

        // Root should change again
        assert_ne!(new_root, final_root);
    }

    #[test]
    fn test_user_deposit_processing() {
        let mut state = GlobalLoanState::new();
        let user_id = [1u8; 32];
        let wallet_address = [2u8; 32];

        // Register user first
        let user_registration = create_test_user_registration(user_id, 1);
        state.process_transaction(&user_registration).unwrap();

        // Create deposit transaction
        let mut deposit_metadata = vec![0u8; 64];
        deposit_metadata[0..32].copy_from_slice(&wallet_address); // First 32 bytes = wallet address

        let deposit_tx = LoanTransaction {
            transaction_id: [10u8; 32],
            user_id,
            loan_id: [0u8; 32], // Zero for non-loan transactions
            transaction_type: TransactionType::UserDeposit,
            amount: 50000, // $500 deposit
            created_at: 1_640_995_400,
            metadata: MetadataArray::from_bytes(&deposit_metadata),
        };

        // Process deposit transaction
        state.process_transaction(&deposit_tx).unwrap();

        // Verify user balance was updated
        assert_eq!(state.get_user_balance(&user_id), 50000);

        // Verify balance update was created
        let balance_history = state.get_user_balance_history(&user_id, None);
        assert_eq!(balance_history.len(), 1);

        let balance_update = balance_history[0];
        assert_eq!(
            balance_update.update_type,
            BalanceUpdateType::CreditFromDeposit
        );
        assert_eq!(balance_update.amount, 50000);
        assert_eq!(balance_update.previous_balance, 0);
        assert_eq!(balance_update.new_balance, 50000);

        // Verify transaction counter was incremented
        assert_eq!(state.transaction_count, 2); // 1 registration + 1 deposit
    }

    #[test]
    fn test_deposit_validation() {
        // Test that deposit with zero amount fails
        let deposit_with_zero_amount = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32],
            transaction_type: TransactionType::UserDeposit,
            amount: 0, // Should fail validation
            created_at: 1_640_995_200,
            metadata: MetadataArray::empty(),
        };

        assert!(validate_transaction(&deposit_with_zero_amount).is_err());

        // Test that deposit with non-zero loan_id fails
        let deposit_with_loan_id = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [1u8; 32], // Should be zero for deposits
            transaction_type: TransactionType::UserDeposit,
            amount: 50000,
            created_at: 1_640_995_200,
            metadata: MetadataArray::empty(),
        };

        assert!(validate_transaction(&deposit_with_loan_id).is_err());

        // Test valid deposit
        let valid_deposit = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            loan_id: [0u8; 32], // Zero for deposits
            transaction_type: TransactionType::UserDeposit,
            amount: 50000,
            created_at: 1_640_995_200,
            metadata: MetadataArray::empty(),
        };

        assert!(validate_transaction(&valid_deposit).is_ok());
    }

    #[test]
    fn test_mixed_batch_with_deposits() {
        let mut state = GlobalLoanState::new();

        let depositor_id = [1u8; 32];
        let borrower_id = [2u8; 32];
        let lender_id = [3u8; 32];
        let loan_id = [10u8; 32];

        // Create a batch with mixed transaction types including deposits
        let base_timestamp = 1_640_995_200;

        let batch = LoanBatch {
            batch_id: [100u8; 32],
            transactions: vec![
                // Register users (use proper sequencing)
                {
                    let mut metadata = MetadataArray::empty();
                    metadata.as_mut_bytes()[0] = 1; // UserType::Investor
                    LoanTransaction {
                        transaction_id: [1u8; 32],
                        user_id: depositor_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::UserRegistration,
                        amount: 0,
                        created_at: base_timestamp + 1000,
                        metadata,
                    }
                },
                {
                    let mut metadata = MetadataArray::empty();
                    metadata.as_mut_bytes()[0] = 1; // UserType::Investor
                    LoanTransaction {
                        transaction_id: [2u8; 32],
                        user_id: borrower_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::UserRegistration,
                        amount: 0,
                        created_at: base_timestamp + 1100,
                        metadata,
                    }
                },
                {
                    let mut metadata = MetadataArray::empty();
                    metadata.as_mut_bytes()[0] = 2; // UserType::Lender
                    LoanTransaction {
                        transaction_id: [3u8; 32],
                        user_id: lender_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::UserRegistration,
                        amount: 0,
                        created_at: base_timestamp + 1200,
                        metadata,
                    }
                },
                // KYC verification
                {
                    let mut metadata = MetadataArray::empty();
                    metadata.as_mut_bytes()[0] = 1; // approved
                    LoanTransaction {
                        transaction_id: [4u8; 32],
                        user_id: depositor_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::KycVerification,
                        amount: 0,
                        created_at: base_timestamp + 2000,
                        metadata,
                    }
                },
                {
                    let mut metadata = MetadataArray::empty();
                    metadata.as_mut_bytes()[0] = 1; // approved
                    LoanTransaction {
                        transaction_id: [5u8; 32],
                        user_id: borrower_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::KycVerification,
                        amount: 0,
                        created_at: base_timestamp + 2100,
                        metadata,
                    }
                },
                {
                    let mut metadata = MetadataArray::empty();
                    metadata.as_mut_bytes()[0] = 1; // approved
                    LoanTransaction {
                        transaction_id: [6u8; 32],
                        user_id: lender_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::KycVerification,
                        amount: 0,
                        created_at: base_timestamp + 2200,
                        metadata,
                    }
                },
                // User deposits
                {
                    let mut deposit_metadata = vec![0u8; 64];
                    let wallet_address = [99u8; 32];
                    deposit_metadata[0..32].copy_from_slice(&wallet_address);

                    LoanTransaction {
                        transaction_id: [20u8; 32],
                        user_id: depositor_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::UserDeposit,
                        amount: 100000, // $1000 deposit
                        created_at: base_timestamp + 3000,
                        metadata: MetadataArray::from_bytes(&deposit_metadata),
                    }
                },
                // Another deposit by lender
                {
                    let mut deposit_metadata = vec![0u8; 64];
                    let wallet_address = [88u8; 32];
                    deposit_metadata[0..32].copy_from_slice(&wallet_address);

                    LoanTransaction {
                        transaction_id: [21u8; 32],
                        user_id: lender_id,
                        loan_id: [0u8; 32],
                        transaction_type: TransactionType::UserDeposit,
                        amount: 150000, // $1500 deposit
                        created_at: base_timestamp + 4000,
                        metadata: MetadataArray::from_bytes(&deposit_metadata),
                    }
                },
                // Loan origination
                {
                    let mut loan_metadata = vec![0u8; 64];
                    loan_metadata[0..32].copy_from_slice(&borrower_id);
                    loan_metadata[32..64].copy_from_slice(&lender_id);

                    LoanTransaction {
                        transaction_id: [22u8; 32],
                        user_id: lender_id,
                        loan_id,
                        transaction_type: TransactionType::LoanOrigination,
                        amount: 50000,
                        created_at: base_timestamp + 5000,
                        metadata: MetadataArray::from_bytes(&loan_metadata),
                    }
                },
            ],
            previous_state_root: [0u8; 32],
            created_at: base_timestamp,
        };

        // Process the batch
        let result = state.process_batch(&batch);
        if let Err(e) = &result {
            println!("Batch processing failed: {:?}", e);
        }
        assert!(result.is_ok());

        // Verify all users exist and are KYC approved
        assert_eq!(state.users.len(), 3);
        assert!(state.get_user(&depositor_id).unwrap().is_kyc_approved());
        assert!(state.get_user(&borrower_id).unwrap().is_kyc_approved());
        assert!(state.get_user(&lender_id).unwrap().is_kyc_approved());

        // Verify user balances from deposits
        assert_eq!(state.get_user_balance(&depositor_id), 100000); // $1000
                                                                   // Lender should have 150000 - 50000 (loan origination) = 100000
        assert_eq!(state.get_user_balance(&lender_id), 100000); // $1000 after loan
                                                                // Borrower should have 50000 from loan origination
        assert_eq!(state.get_user_balance(&borrower_id), 50000); // $500

        // Verify loan exists
        let loan = state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.principal_amount, 50000);

        // Verify correct number of balance updates
        // Depositor: 1 deposit
        // Lender: 1 deposit + 1 debit from origination
        // Borrower: 1 credit from origination
        let depositor_history = state.get_user_balance_history(&depositor_id, None);
        let lender_history = state.get_user_balance_history(&lender_id, None);
        let borrower_history = state.get_user_balance_history(&borrower_id, None);

        assert_eq!(depositor_history.len(), 1); // 1 deposit
        assert_eq!(lender_history.len(), 2); // 1 deposit + 1 origination debit
        assert_eq!(borrower_history.len(), 1); // 1 origination credit

        // Verify deposit updates have correct type
        assert_eq!(
            depositor_history[0].update_type,
            BalanceUpdateType::CreditFromDeposit
        );
        assert_eq!(
            lender_history[1].update_type,
            BalanceUpdateType::CreditFromDeposit
        ); // Second is the deposit (sorted by timestamp desc)

        // Total transaction count should be 9 (3 registrations + 3 KYC verifications + 2 deposits + 1 loan origination)
        assert_eq!(state.transaction_count, 9);

        // Verify total system value calculation (what ZK program will use)
        let total_system_value = state.compute_total_user_balance();
        // Should equal total deposits: $1000 (depositor) + $1500 (lender) = $2500
        assert_eq!(total_system_value, 250000); // $2500 in cents

        // Verify this equals sum of individual balances
        let sum_of_balances = state.get_user_balance(&depositor_id)
            + state.get_user_balance(&lender_id)
            + state.get_user_balance(&borrower_id);
        assert_eq!(total_system_value, sum_of_balances);

        // Outstanding loan balance is separate and should be $500
        let loan_balance_total = state.compute_total_balance();
        assert_eq!(loan_balance_total, 50000); // $500 in cents
    }

    #[test]
    fn test_compute_total_balance() {
        let mut state = GlobalLoanState::new();

        // Initially, total balance should be 0
        assert_eq!(state.compute_total_balance(), 0);

        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];

        // Set up KYC'd users
        setup_kycd_users(&mut state, borrower_id, lender_id);

        // Create multiple loans
        let loan1_id = [10u8; 32];
        let loan2_id = [11u8; 32];
        let loan3_id = [12u8; 32];

        // Loan 1: Active with $100 outstanding balance
        let loan1 = crate::loan::Loan {
            loan_id: loan1_id,
            borrower_id,
            lender_id,
            principal_amount: 10000, // $100
            outstanding_balance: 10000,
            interest_rate: 750,
            term_months: 36,
            loan_status: LoanStatus::Active,
            originated_at: 1_640_995_200,
            funded_at: 1_640_995_200,
            maturity_date: 1_640_995_200 + (36 * 30 * 24 * 60 * 60),
        };

        // Loan 2: Originated with $200 outstanding balance
        let loan2 = crate::loan::Loan {
            loan_id: loan2_id,
            borrower_id,
            lender_id,
            principal_amount: 20000, // $200
            outstanding_balance: 20000,
            interest_rate: 750,
            term_months: 36,
            loan_status: LoanStatus::Originated,
            originated_at: 1_640_995_200,
            funded_at: 0,
            maturity_date: 1_640_995_200 + (36 * 30 * 24 * 60 * 60),
        };

        // Loan 3: PaidOff (should not count toward balance)
        let loan3 = crate::loan::Loan {
            loan_id: loan3_id,
            borrower_id,
            lender_id,
            principal_amount: 30000, // $300
            outstanding_balance: 0,
            interest_rate: 750,
            term_months: 36,
            loan_status: LoanStatus::PaidOff,
            originated_at: 1_640_995_200,
            funded_at: 1_640_995_200,
            maturity_date: 1_640_995_200 + (36 * 30 * 24 * 60 * 60),
        };

        state.loans.insert(loan1_id, loan1);
        state.loans.insert(loan2_id, loan2);
        state.loans.insert(loan3_id, loan3);

        // Total balance should be $100 + $200 = $300 (loan3 doesn't count as it's paid off)
        assert_eq!(state.compute_total_balance(), 30000);

        // Make a payment on loan1 to reduce its outstanding balance
        state.loans.get_mut(&loan1_id).unwrap().outstanding_balance = 5000; // $50 remaining

        // Total balance should now be $50 + $200 = $250
        assert_eq!(state.compute_total_balance(), 25000);

        // Pay off loan2
        state.loans.get_mut(&loan2_id).unwrap().outstanding_balance = 0;
        state.loans.get_mut(&loan2_id).unwrap().loan_status = LoanStatus::PaidOff;

        // Total balance should now be just $50
        assert_eq!(state.compute_total_balance(), 5000);
    }

    #[test]
    fn test_user_withdrawal_processing() {
        let mut state = GlobalLoanState::new();
        let user_id = [1u8; 32];
        let recipient_address = [42u8; 32]; // Mock recipient wallet address

        // Register and KYC user
        let user_reg = create_test_user_registration(user_id, 1);
        state.process_transaction(&user_reg).unwrap();

        let kyc_submission = create_test_kyc_submission(user_id);
        state.process_transaction(&kyc_submission).unwrap();

        let kyc_verification = create_test_kyc_verification(user_id, true);
        state.process_transaction(&kyc_verification).unwrap();

        // First, user needs to have a balance via deposit
        let deposit_tx = create_test_deposit(user_id, [99u8; 32], 100000); // $1000 deposit
        state.process_transaction(&deposit_tx).unwrap();

        // Verify user has balance
        assert_eq!(state.balance_state.get_balance(&user_id), 100000);

        // Create withdrawal transaction
        let withdrawal_tx = create_test_withdrawal(user_id, recipient_address, 25000); // $250 withdrawal
        let result = state.process_transaction(&withdrawal_tx);

        // Should succeed
        assert!(result.is_ok());

        // Verify balance was debited
        assert_eq!(state.balance_state.get_balance(&user_id), 75000); // $750 remaining

        // Verify balance update was created
        let updates = state.balance_state.get_user_update_history(&user_id, None);
        assert_eq!(updates.len(), 2); // Deposit + withdrawal

        let withdrawal_update = updates
            .iter()
            .find(|u| u.update_type == BalanceUpdateType::DebitFromWithdrawal)
            .unwrap();
        assert_eq!(withdrawal_update.amount, 25000);
        assert_eq!(withdrawal_update.previous_balance, 100000);
        assert_eq!(withdrawal_update.new_balance, 75000);
        assert!(withdrawal_update
            .metadata
            .as_ref()
            .unwrap()
            .contains("Withdrawal to wallet 2a2a2a2a"));
    }

    #[test]
    fn test_withdrawal_insufficient_balance_fails() {
        let mut state = GlobalLoanState::new();
        let user_id = [1u8; 32];
        let recipient_address = [42u8; 32];

        // Register and KYC user
        let user_reg = create_test_user_registration(user_id, 1);
        state.process_transaction(&user_reg).unwrap();

        let kyc_submission = create_test_kyc_submission(user_id);
        state.process_transaction(&kyc_submission).unwrap();

        let kyc_verification = create_test_kyc_verification(user_id, true);
        state.process_transaction(&kyc_verification).unwrap();

        // User has no balance (no deposit)
        assert_eq!(state.balance_state.get_balance(&user_id), 0);

        // Try to withdraw $100
        let withdrawal_tx = create_test_withdrawal(user_id, recipient_address, 10000);
        let result = state.process_transaction(&withdrawal_tx);

        // Should fail with insufficient balance
        assert!(matches!(
            result,
            Err(ProcessingError::UserError(UserError::KycNotApproved { .. }))
                | Err(ProcessingError::BalanceError(
                    BalanceError::InsufficientBalance { .. }
                ))
        ));
    }

    #[test]
    fn test_withdrawal_without_kyc_fails() {
        let mut state = GlobalLoanState::new();
        let user_id = [1u8; 32];
        let recipient_address = [42u8; 32];

        // Register user but don't KYC them
        let user_reg = create_test_user_registration(user_id, 1);
        state.process_transaction(&user_reg).unwrap();

        // Give user some balance via deposit (deposits don't require KYC)
        let deposit_tx = create_test_deposit(user_id, [99u8; 32], 100000);
        state.process_transaction(&deposit_tx).unwrap();

        // Try to withdraw without KYC
        let withdrawal_tx = create_test_withdrawal(user_id, recipient_address, 25000);
        let result = state.process_transaction(&withdrawal_tx);

        // Should fail with KYC not approved
        assert!(matches!(
            result,
            Err(ProcessingError::UserError(UserError::KycNotApproved { .. }))
        ));

        // Balance should remain unchanged
        assert_eq!(state.balance_state.get_balance(&user_id), 100000);
    }

    #[test]
    fn test_withdrawal_batch_processing() {
        let mut state = GlobalLoanState::new();
        let user1_id = [1u8; 32];
        let user2_id = [2u8; 32];
        let recipient1 = [42u8; 32];
        let recipient2 = [43u8; 32];

        // Setup two KYC'd users with balances
        setup_kycd_users(&mut state, user1_id, user2_id);

        // Give both users deposits (after KYC setup which uses timestamps up to base + 3100)
        let deposit1 =
            create_test_deposit_with_timestamp(user1_id, [99u8; 32], 100000, 1_640_995_200 + 4000); // $1000
        let deposit2 =
            create_test_deposit_with_timestamp(user2_id, [98u8; 32], 200000, 1_640_995_200 + 4100); // $2000
        state.process_transaction(&deposit1).unwrap();
        state.process_transaction(&deposit2).unwrap();

        // Create batch with withdrawals from both users
        let withdrawal1 = create_test_withdrawal_with_timestamp(
            user1_id,
            recipient1,
            30000,
            1_640_995_200 + 5000,
        ); // $300
        let withdrawal2 = create_test_withdrawal_with_timestamp(
            user2_id,
            recipient2,
            50000,
            1_640_995_200 + 5100,
        ); // $500

        let batch = LoanBatch {
            batch_id: [99u8; 32],
            transactions: vec![withdrawal1, withdrawal2],
            previous_state_root: [0u8; 32],
            created_at: 1_640_995_200 + 5200,
        };

        // Process batch
        let result = state.process_batch(&batch);
        assert!(result.is_ok());

        // Verify both balances were debited correctly
        assert_eq!(state.balance_state.get_balance(&user1_id), 70000); // $700 remaining
        assert_eq!(state.balance_state.get_balance(&user2_id), 150000); // $1500 remaining

        // Verify total system balance decreased correctly
        assert_eq!(state.compute_total_user_balance(), 220000); // $2200 total
    }

    // Helper function to create test KYC submission transaction
    fn create_test_kyc_submission(user_id: [u8; 32]) -> LoanTransaction {
        use crate::transactions::MetadataArray;

        LoanTransaction {
            transaction_id: [6u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: 1_640_995_200,
            metadata: MetadataArray::empty(),
        }
    }

    // Helper function to create test KYC verification transaction
    fn create_test_kyc_verification(user_id: [u8; 32], approved: bool) -> LoanTransaction {
        use crate::transactions::MetadataArray;

        let mut metadata = MetadataArray::empty();
        metadata.as_mut_bytes()[0] = if approved { 1 } else { 0 };

        LoanTransaction {
            transaction_id: [7u8; 32],
            user_id,
            loan_id: [0u8; 32],
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1_640_995_210,
            metadata,
        }
    }

    // Helper function to create test withdrawal transaction
    fn create_test_withdrawal(
        user_id: [u8; 32],
        recipient_address: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        create_test_withdrawal_with_timestamp(user_id, recipient_address, amount, 1_640_995_250)
    }

    // Helper function to create test withdrawal transaction with custom timestamp
    fn create_test_withdrawal_with_timestamp(
        user_id: [u8; 32],
        recipient_address: [u8; 32],
        amount: u64,
        timestamp: u64,
    ) -> LoanTransaction {
        use crate::transactions::MetadataArray;

        let mut metadata_bytes = vec![0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&recipient_address);

        LoanTransaction {
            transaction_id: [3u8; 32], // Mock transaction ID
            user_id,
            loan_id: [0u8; 32], // No loan associated
            transaction_type: TransactionType::UserWithdrawal,
            amount,
            created_at: timestamp,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    // Helper function to create test deposit transaction
    fn create_test_deposit(
        user_id: [u8; 32],
        wallet_address: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        create_test_deposit_with_timestamp(user_id, wallet_address, amount, 1_640_995_220)
    }

    // Helper function to create test deposit transaction with custom timestamp
    fn create_test_deposit_with_timestamp(
        user_id: [u8; 32],
        wallet_address: [u8; 32],
        amount: u64,
        timestamp: u64,
    ) -> LoanTransaction {
        use crate::transactions::MetadataArray;

        let mut metadata_bytes = vec![0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&wallet_address);

        LoanTransaction {
            transaction_id: [4u8; 32], // Mock transaction ID
            user_id,
            loan_id: [0u8; 32], // No loan associated
            transaction_type: TransactionType::UserDeposit,
            amount,
            created_at: timestamp,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    #[test]
    fn test_pool_creation() {
        let mut state = GlobalLoanState::new();
        let investor_id = [1u8; 32];
        let pool_id = [10u8; 32];

        // Register investor first
        let investor_registration = create_test_user_registration(investor_id, 1);
        state.process_transaction(&investor_registration).unwrap();

        // Create investor registration transaction
        let investor_reg = LoanTransaction {
            transaction_id: [20u8; 32],
            user_id: investor_id,
            loan_id: [0u8; 32], // Zero for non-loan transactions
            transaction_type: TransactionType::InvestorRegistration,
            amount: 0,
            created_at: 1_640_995_300,
            metadata: MetadataArray::empty(),
        };

        // Process investor registration
        state.process_transaction(&investor_reg).unwrap();

        // Add KYC verification (required for pool creation)
        let mut kyc_metadata = vec![0u8; 64];
        kyc_metadata[0] = 1; // 1 = approval, 0 = rejection

        let kyc_verification = LoanTransaction {
            transaction_id: [25u8; 32],
            user_id: investor_id,
            loan_id: [0u8; 32], // Zero for non-loan transactions
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1_640_995_350,
            metadata: MetadataArray::from_bytes(&kyc_metadata),
        };

        state.process_transaction(&kyc_verification).unwrap();

        // Verify user type updated to investor and KYC approved
        let user = state.users.get(&investor_id).unwrap();
        assert_eq!(user.user_type, crate::user::UserType::Investor);
        assert_eq!(user.kyc_status, crate::user::KycStatus::Approved);

        // Create pool creation transaction
        let mut pool_metadata = vec![0u8; 64];
        // Pack pool configuration: max_ltv_ratio (8000 = 80%), max_interest_rate (1200 = 12%), repayment_period_days (365)
        let max_ltv_ratio = 8000u64;
        let max_interest_rate = 1200u64;
        let repayment_period_days = 365u32;

        pool_metadata[0..8].copy_from_slice(&max_ltv_ratio.to_le_bytes());
        pool_metadata[8..16].copy_from_slice(&max_interest_rate.to_le_bytes());
        pool_metadata[16..20].copy_from_slice(&repayment_period_days.to_le_bytes());

        let pool_creation = LoanTransaction {
            transaction_id: [30u8; 32],
            user_id: investor_id,
            loan_id: pool_id, // Using loan_id field as pool_id
            transaction_type: TransactionType::PoolCreation,
            amount: 0,
            created_at: 1_640_995_400,
            metadata: MetadataArray::from_bytes(&pool_metadata),
        };

        // Process pool creation
        state.process_transaction(&pool_creation).unwrap();

        // Verify pool was created
        let pool = state.pools.get_pool(&pool_id).unwrap();
        assert_eq!(pool.investor_id, investor_id);
        assert_eq!(pool.max_ltv_ratio, 8000);
        assert_eq!(pool.max_interest_rate, 1200);
        assert_eq!(pool.repayment_period_days, 365);
        assert_eq!(pool.status, crate::pool::PoolStatus::Pending);

        // Verify pool state integrity
        assert_eq!(state.pools.total_pools(), 1);
        assert_eq!(state.pools.active_pools(), 0); // Still pending

        // Verify transaction counter was incremented
        assert_eq!(state.transaction_count, 4); // 1 user registration + 1 investor registration + 1 kyc verification + 1 pool creation
    }
}
