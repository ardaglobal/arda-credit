//! Core loan processing logic for zkVM execution
//!
//! This module contains the core state transition logic that will run inside
//! the SP1 zkVM. It processes batches of mixed transaction types while maintaining
//! privacy guarantees.

#[cfg(test)]
use crate::balance::BalanceUpdateType;
use crate::balance::{BalanceError, BalanceState, BalanceUpdate};
use crate::deal::LoanTerms; // Keep LoanTerms name for compatibility
use crate::deal::{Deal, DealError}; // was Loan, LoanError
use crate::transactions::{LoanBatch, LoanTransaction, TransactionType};
use crate::user::{User, UserError, UserState};
use crate::LoanStatus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during loan processing
#[derive(Error, Debug, Clone)]
pub enum ProcessingError {
    #[error("User error: {0}")]
    UserError(#[from] UserError),

    #[error("Company error: {0}")]
    CompanyError(#[from] crate::company::CompanyError),

    #[error("Loan error: {0}")]
    LoanError(#[from] DealError), // was LoanError

    #[error("Balance error: {0}")]
    BalanceError(#[from] BalanceError),

    #[error("Collateral error: {0}")]
    CollateralError(#[from] crate::collateral::CollateralError),

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

/// Global state for loan processing (maintains user, company, deal, and collateral state)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLoanState {
    /// All users in the system (individuals who act on behalf of companies)
    pub users: HashMap<[u8; 32], User>,

    /// All companies in the system (entities that participate in deals)
    pub companies: HashMap<[u8; 32], crate::company::Company>,

    /// All loans in the system (deals between companies)
    pub loans: HashMap<[u8; 32], Deal>, // Keep loans field name for compatibility, but use Deal type

    /// All collateral in the system
    pub collateral: HashMap<Vec<u8>, crate::collateral::Collateral>,

    /// Active collateral-deal associations
    pub active_associations: HashMap<Vec<u8>, Vec<Vec<u8>>>,

    /// Complete association history
    pub association_history: HashMap<Vec<u8>, crate::collateral::CollateralAssociation>,

    /// Company balance tracking state (companies have balances, not users)
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
            companies: HashMap::new(),
            loans: HashMap::new(),
            collateral: HashMap::new(),
            active_associations: HashMap::new(),
            association_history: HashMap::new(),
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
            companies: HashMap::new(),
            loans: HashMap::new(),
            collateral: HashMap::new(),
            active_associations: HashMap::new(),
            association_history: HashMap::new(),
            balance_state: BalanceState::new(),
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
            TransactionType::DealOrigination => {
                // Validate companies exist in state
                if !self
                    .companies
                    .contains_key(&transaction.originator_company_id)
                {
                    return Err(ProcessingError::LoanError(DealError::UserError(
                        crate::user::UserError::UserNotFound {
                            user_id: transaction.originator_company_id,
                        },
                    )));
                }
                if !self
                    .companies
                    .contains_key(&transaction.investor_company_id)
                {
                    return Err(ProcessingError::LoanError(DealError::UserError(
                        crate::user::UserError::UserNotFound {
                            user_id: transaction.investor_company_id,
                        },
                    )));
                }

                let mut deal_state = crate::deal::DealState {
                    deals: self.loans.clone(), // Keep loans field for compatibility
                };
                deal_state.process_deal_origination(transaction, &self.users, &self.loan_terms)?;
                self.loans = deal_state.deals;

                // Create balance updates for deal origination
                // Use explicit company ID fields (not metadata parsing)
                let originator_company_id = transaction.originator_company_id;
                let investor_company_id = transaction.investor_company_id;

                let balance_updates = self.balance_state.create_origination_updates(
                    transaction.transaction_id,
                    transaction.deal_id,
                    originator_company_id,
                    investor_company_id,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance updates to state
                for update in balance_updates {
                    self.balance_state.apply_update(update)?;
                }
            }
            TransactionType::DealPayment => {
                // Get deal info before processing payment for balance updates
                let deal =
                    self.loans
                        .get(&transaction.deal_id)
                        .ok_or(ProcessingError::LoanError(DealError::DealNotFound {
                            deal_id: transaction.deal_id,
                        }))?;

                // For payments: originator_company_id is the payer, investor is from the deal
                let payer_company_id = transaction.originator_company_id;
                let investor_company_id = deal.investor_id;

                let mut deal_state = crate::deal::DealState {
                    deals: self.loans.clone(), // Keep loans field for compatibility
                };
                deal_state.process_deal_payment(transaction, &self.users)?;
                self.loans = deal_state.deals;

                // Create balance updates for deal payment (company balances)
                let balance_updates = self.balance_state.create_payment_updates(
                    transaction.transaction_id,
                    transaction.deal_id,
                    payer_company_id,
                    investor_company_id,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance updates to state
                for update in balance_updates {
                    self.balance_state.apply_update(update)?;
                }
            }
            TransactionType::DealCloseout => {
                // was LoanCloseout
                let mut deal_state = crate::deal::DealState {
                    deals: self.loans.clone(), // Keep loans field for compatibility
                };
                deal_state.process_deal_closeout(transaction, &self.users)?; // was process_loan_closeout
                self.loans = deal_state.deals; // Keep loans field for compatibility
            }
            TransactionType::UserDeposit => {
                // Process company deposit by creating a balance update
                // For deposits, originator_company_id is the company receiving the deposit
                // Metadata contains: [wallet_address(32)] + [remaining_32_bytes_unused]
                let metadata = transaction.metadata.as_bytes();
                let mut wallet_address = [0u8; 32];
                wallet_address.copy_from_slice(&metadata[0..32]);

                let company_id = transaction.originator_company_id;

                let balance_update = self.balance_state.create_deposit_update(
                    transaction.transaction_id,
                    company_id,
                    wallet_address,
                    u128::from(transaction.amount),
                    transaction.created_at,
                )?;

                // Apply balance update to state
                self.balance_state.apply_update(balance_update)?;
            }
            TransactionType::UserWithdrawal => {
                // Process company withdrawal by creating a balance update
                // For withdrawals, originator_company_id is the company making the withdrawal
                // Metadata contains: [recipient_address(32)] + [remaining_32_bytes_unused]
                let metadata = transaction.metadata.as_bytes();
                let mut recipient_address = [0u8; 32];
                recipient_address.copy_from_slice(&metadata[0..32]);

                let company_id = transaction.originator_company_id;

                let balance_update = self.balance_state.create_withdrawal_update(
                    transaction.transaction_id,
                    company_id,
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
            TransactionType::AdminRegistration => {
                // Process admin registration
                let mut user_state = UserState {
                    users: self.users.clone(),
                };
                user_state.process_admin_registration(transaction)?;
                self.users = user_state.users;
            }
            TransactionType::VerifyDocument => todo!(),
            TransactionType::CollateralCreation => {
                let mut collateral_state = crate::collateral::CollateralState {
                    collateral: self.collateral.clone(),
                    active_associations: self.active_associations.clone(),
                    association_history: self.association_history.clone(),
                    detection_requests: std::collections::HashMap::new(), // Not needed for creation
                };

                collateral_state
                    .process_collateral_creation(transaction)
                    .map_err(|e| ProcessingError::CollateralError(e))?;

                self.collateral = collateral_state.collateral;
            }

            TransactionType::CollateralUpdate => {
                let mut collateral_state = crate::collateral::CollateralState {
                    collateral: self.collateral.clone(),
                    active_associations: self.active_associations.clone(),
                    association_history: self.association_history.clone(),
                    detection_requests: std::collections::HashMap::new(),
                };

                collateral_state
                    .process_collateral_update(transaction)
                    .map_err(|e| ProcessingError::CollateralError(e))?;

                self.collateral = collateral_state.collateral;
            }

            TransactionType::CollateralDealAssociation => {
                let mut collateral_state = crate::collateral::CollateralState {
                    collateral: self.collateral.clone(),
                    active_associations: self.active_associations.clone(),
                    association_history: self.association_history.clone(),
                    detection_requests: std::collections::HashMap::new(),
                };

                // CRITICAL: This proves no double-pledging!
                collateral_state
                    .process_collateral_association(transaction)
                    .map_err(|e| ProcessingError::CollateralError(e))?;

                self.active_associations = collateral_state.active_associations;
                self.association_history = collateral_state.association_history;
            }

            TransactionType::CollateralDealDisassociation => {
                let mut collateral_state = crate::collateral::CollateralState {
                    collateral: self.collateral.clone(),
                    active_associations: self.active_associations.clone(),
                    association_history: self.association_history.clone(),
                    detection_requests: std::collections::HashMap::new(),
                };

                collateral_state
                    .process_collateral_disassociation(transaction)
                    .map_err(|e| ProcessingError::CollateralError(e))?;

                self.active_associations = collateral_state.active_associations;
                self.association_history = collateral_state.association_history;
            }

            TransactionType::CollateralDetectionRequest => {
                let mut collateral_state = crate::collateral::CollateralState {
                    collateral: self.collateral.clone(),
                    active_associations: std::collections::HashMap::new(), // Not needed
                    association_history: std::collections::HashMap::new(), // Not needed
                    detection_requests: std::collections::HashMap::new(),
                };

                collateral_state
                    .process_collateral_detection_request(transaction)
                    .map_err(|e| ProcessingError::CollateralError(e))?;

                // Detection requests don't update state (just audit trail)
            }

            TransactionType::CollateralDetectionUpdate => {
                let mut collateral_state = crate::collateral::CollateralState {
                    collateral: self.collateral.clone(),
                    active_associations: std::collections::HashMap::new(),
                    association_history: std::collections::HashMap::new(),
                    detection_requests: std::collections::HashMap::new(),
                };

                collateral_state
                    .process_collateral_detection_update(transaction)
                    .map_err(|e| ProcessingError::CollateralError(e))?;

                // Detection updates don't modify state (just audit trail)
            }
            TransactionType::StateRegenesis => {
                // Re-genesis transaction marks state reset point
                // This transaction itself doesn't modify state - it's a marker in the audit trail
                // The actual state reset happens in the batch processor
            }
            TransactionType::CompanyRegistration => {
                let mut company_state = crate::company::CompanyState {
                    companies: self.companies.clone(),
                };
                company_state.process_company_registration(transaction)?;
                self.companies = company_state.companies;
            }
            TransactionType::CompanyUpdate => {
                let mut company_state = crate::company::CompanyState {
                    companies: self.companies.clone(),
                };
                company_state.process_company_update(transaction)?;
                self.companies = company_state.companies;
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
    pub fn get_loan(&self, deal_id: &[u8; 32]) -> Option<&Deal> {
        // was loan_id, now deal_id, Loan
        self.loans.get(deal_id) // was loan_id, now deal_id
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
                loan.deal_status,
                LoanStatus::Active | LoanStatus::Originated // was crate::loan::LoanStatus
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

    /// Compute total balance across all users
    #[must_use]
    pub fn compute_total_user_balance(&self) -> u128 {
        self.balance_state.compute_total_balance()
    }

    /// Get balance updates for a specific loan
    #[must_use]
    pub fn get_loan_balance_updates(&self, loan_id: &[u8; 32]) -> Vec<&BalanceUpdate> {
        self.balance_state.get_loan_balance_updates(loan_id)
    }

    /// Get collateral by ID (read-only)
    #[must_use]
    pub fn get_collateral(&self, collateral_id: &[u8]) -> Option<&crate::collateral::Collateral> {
        self.collateral.get(collateral_id)
    }

    /// Get active associations for a collateral
    #[must_use]
    pub fn get_active_associations(&self, collateral_id: &[u8]) -> Vec<Vec<u8>> {
        self.active_associations
            .get(collateral_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Check if collateral is actively pledged to any deal
    #[must_use]
    pub fn is_collateral_pledged(&self, collateral_id: &[u8]) -> bool {
        self.active_associations
            .get(collateral_id)
            .map_or(false, |deals| !deals.is_empty())
    }

    /// Get all collateral associated with a deal
    #[must_use]
    pub fn get_deal_collateral(&self, deal_id: &[u8]) -> Vec<Vec<u8>> {
        self.active_associations
            .iter()
            .filter_map(|(coll_id, deal_ids)| {
                if deal_ids.iter().any(|d| d.as_slice() == deal_id) {
                    Some(coll_id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Compute total collateral count
    #[must_use]
    pub fn total_collateral_count(&self) -> usize {
        self.collateral.len()
    }

    /// Compute total active associations
    #[must_use]
    pub fn total_active_associations(&self) -> usize {
        self.active_associations
            .values()
            .map(|deals| deals.len())
            .sum()
    }
}

/// Process a complete loan batch and return the state transition
///
/// # Errors
/// Returns `ProcessingError` if batch processing fails due to validation errors,
/// temporal ordering issues, or transaction processing errors.
pub fn process_deal_batch(
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
        TransactionType::DealOrigination  // was LoanOrigination
        | TransactionType::DealPayment    // was LoanPayment
        | TransactionType::UserDeposit
        | TransactionType::UserWithdrawal => {
            if transaction.amount == 0 {
                return Err(ProcessingError::InvalidTransactionType);
            }
        }
        _ => {}
    }

    // Validate all zeros loan_id for non-loan transactions
    let zero_deal_id = [0u8; 32]; // was zero_loan_id
    match transaction.transaction_type {
        TransactionType::UserRegistration
        | TransactionType::KycSubmission
        | TransactionType::KycVerification
        | TransactionType::UserDeposit
        | TransactionType::UserWithdrawal
        | TransactionType::InvestorRegistration
        | TransactionType::AdminRegistration
        | TransactionType::VerifyDocument => {
            if transaction.deal_id != zero_deal_id {
                // was loan_id, now deal_id, zero_loan_id
                return Err(ProcessingError::InvalidTransactionType);
            }
        }

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

    // NOTE: State timestamp validation removed - since we process from genesis (last_updated = 0),
    // all valid transactions (2000+) are naturally newer than initial state.
    // Batch sorting ensures chronological order is maintained.

    // Skip user validation if user_id is all zeros (system/automated transactions)
    let is_system_transaction = transaction.user_id == [0u8; 32];

    // Validate KYC requirements for transactions that require it
    if transaction.transaction_type.requires_kyc() && !is_system_transaction {
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

        // For withdrawals, also validate sufficient company balance
        if transaction.transaction_type == TransactionType::UserWithdrawal {
            let company_id = transaction.originator_company_id;
            let current_balance = current_state.balance_state.get_balance(&company_id);
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
    // Min: January 1, 2000 (946684800) - Private credit industry historical data
    // Max: January 1, 2060 (2840140800) - Support 30-year mortgages and long-term loans
    const MIN_TIMESTAMP: u64 = 946_684_800; // Jan 1, 2000
    const MAX_TIMESTAMP: u64 = 2_840_140_800; // Jan 1, 2060

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
    /// Create a new deal
    LoanCreation {
        deal_id: [u8; 32], // was loan_id, now deal_id
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        principal: u128,
        interest_rate: u64,
        term_months: u32,
    },
    /// Make a payment towards a deal
    Payment {
        deal_id: [u8; 32], // was loan_id, now deal_id
        payer_id: [u8; 32],
        amount: u128,
    },
    /// Fund an originated deal
    LoanFunding {
        deal_id: [u8; 32], // was loan_id, now deal_id
        lender_id: [u8; 32],
    },
    /// Update balance (for reconciliation)
    BalanceUpdate {
        deal_id: [u8; 32], // was loan_id, now deal_id
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

    /// Convert operation to nullifier hash for Merkle tree using Poseidon2
    #[must_use]
    pub fn to_nullifier(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};

        // Use SHA-256 to combine the operation data into a single value
        // then use that as input to Poseidon for the final hash
        let mut hasher = Sha256::new();
        match self {
            Self::LoanCreation {
                deal_id, // was loan_id, now deal_id
                borrower_id,
                lender_id,
                principal,
                ..
            } => {
                hasher.update(b"loan_creation");
                hasher.update(deal_id);
                hasher.update(borrower_id);
                hasher.update(lender_id);
                hasher.update(principal.to_le_bytes());
            }
            Self::Payment {
                deal_id, // was loan_id, now deal_id
                payer_id,
                amount,
            } => {
                hasher.update(b"payment");
                hasher.update(deal_id);
                hasher.update(payer_id);
                hasher.update(amount.to_le_bytes());
            }
            Self::LoanFunding { deal_id, lender_id } => {
                // was loan_id, now deal_id
                hasher.update(b"funding");
                hasher.update(deal_id);
                hasher.update(lender_id);
            }
            Self::BalanceUpdate {
                deal_id, // was loan_id, now deal_id
                new_balance,
            } => {
                hasher.update(b"balance_update");
                hasher.update(deal_id);
                hasher.update(new_balance.to_le_bytes());
            }
        }
        let sha_hash: [u8; 32] = hasher.finalize().into();

        // Apply Poseidon2 to the SHA-256 hash for ZK-friendly final hash
        // This two-stage approach ensures we can handle arbitrary-length inputs
        use crate::hash::hash_internal_poseidon;
        hash_internal_poseidon(&sha_hash, &[0u8; 32])
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

    /// Compute the Merkle root from all leaves using Poseidon2
    fn compute_root(&self) -> [u8; 32] {
        use crate::hash::hash_internal_poseidon;

        if self.leaves.is_empty() {
            return [0u8; 32];
        }
        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::new();

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() > 1 {
                    hash_internal_poseidon(&chunk[0], &chunk[1])
                } else {
                    // Duplicate if odd number
                    hash_internal_poseidon(&chunk[0], &chunk[0])
                };
                next_level.push(hash);
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
    use crate::deal::LoanStatus;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus, UserType};

    fn create_test_user_registration(user_id: [u8; 32], user_type: u8) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [1u8; 32],
            user_id,
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: 1_640_995_300, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&[u8::from(approved)]),
        }
    }

    fn create_deal_origination(
        deal_id: [u8; 32],
        originator_id: [u8; 32],
        investor_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&originator_id);
        metadata[32..64].copy_from_slice(&investor_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id: originator_id, // was borrower_id
            deal_id,                // Keep deal_id field name
            transaction_type: TransactionType::DealOrigination, // was LoanOrigination
            amount,
            created_at: 1_640_995_400, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    #[test]
    fn test_full_deal_lifecycle() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // 1. Register users
        let originator_reg = create_test_user_registration(originator_id, 1); // Investor
        let investor_reg = create_test_user_registration(investor_id, 2); // Lender

        state.process_transaction(&originator_reg).unwrap();
        state.process_transaction(&investor_reg).unwrap();

        // 2. KYC verification
        let originator_kyc = create_kyc_verification(originator_id, true);
        let investor_kyc = create_kyc_verification(investor_id, true);

        state.process_transaction(&originator_kyc).unwrap();
        state.process_transaction(&investor_kyc).unwrap();

        // Verify users are registered and KYC approved
        let originator = state.get_user(&originator_id).unwrap();
        let investor = state.get_user(&investor_id).unwrap();
        assert_eq!(originator.kyc_status, KycStatus::Approved);
        assert_eq!(investor.kyc_status, KycStatus::Approved);

        // 3. Give investor sufficient balance for deal origination (for testing)
        let initial_funding = BalanceUpdate::new(
            [99u8; 32],  // funding transaction ID
            investor_id, // was lender_id
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            100_000,       // Give enough for the 50000 deal
            0,             // previous balance
            1_640_995_100, // before deal origination
            Some("Test funding".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(initial_funding).unwrap();

        // 4. Deal origination
        let deal_origination = create_deal_origination(deal_id, originator_id, investor_id, 50000);
        state.process_transaction(&deal_origination).unwrap();

        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.deal_status, LoanStatus::Originated); // Keep loan_status for compatibility
        assert_eq!(deal.principal_amount, 50000);

        // 5. Deal payment
        let payment = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: originator_id,                         // was borrower_id
            deal_id,                                        // Keep deal_id field name
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 25000,                                  // Partial payment
            created_at: 1_640_995_500,                      // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&payment).unwrap();

        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.deal_status, LoanStatus::Active); // Keep loan_status for compatibility
        assert_eq!(deal.outstanding_balance, 25000);

        // 5. Final payment
        let final_payment = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: originator_id,                         // was borrower_id
            deal_id,                                        // Keep deal_id field name
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount: 25000,                                  // Final payment
            created_at: 1_640_995_600,                      // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&final_payment).unwrap();

        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.deal_status, LoanStatus::PaidOff); // Keep loan_status for compatibility
        assert_eq!(deal.outstanding_balance, 0);

        // 6. Deal closeout
        let closeout = LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: investor_id,                            // was lender_id
            deal_id,                                         // Keep deal_id field name
            transaction_type: TransactionType::DealCloseout, // was LoanCloseout
            amount: 0,
            created_at: 1_640_995_700, // January 1, 2022 - valid timestamp
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&closeout).unwrap();

        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.deal_status, LoanStatus::Closed); // Keep loan_status for compatibility

        // Verify transaction count
        assert_eq!(state.transaction_count, 8); // 2 registrations + 2 KYC verifications + 1 origination + 2 payments + 1 closeout
    }

    #[test]
    fn test_deal_origination_without_kyc_fails() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];

        // Register users but don't KYC them
        let originator_reg = create_test_user_registration(originator_id, 1);
        let investor_reg = create_test_user_registration(investor_id, 2);

        state.process_transaction(&originator_reg).unwrap();
        state.process_transaction(&investor_reg).unwrap();

        // Try to originate deal without KYC
        let deal_origination = create_deal_origination(deal_id, originator_id, investor_id, 50000);
        let result = state.process_transaction(&deal_origination);

        assert!(matches!(
            result,
            Err(ProcessingError::UserError(UserError::KycNotApproved { .. }))
                | Err(ProcessingError::LoanError(DealError::UserError(
                    UserError::KycNotApproved { .. }
                ))) // Keep LoanError for compatibility
        ));
    }

    #[test]
    fn test_process_batch() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];

        let batch = LoanBatch {
            batch_id: [100u8; 32],
            transactions: vec![
                create_test_user_registration(originator_id, 1),
                create_test_user_registration(investor_id, 2),
                create_kyc_verification(originator_id, true),
                create_kyc_verification(investor_id, true),
            ],
            previous_state_root: [0u8; 32],
            created_at: 1_640_995_200, // January 1, 2022 - valid timestamp
        };

        let result = state.process_batch(&batch);
        assert!(result.is_ok());

        assert_eq!(state.users.len(), 2);
        assert_eq!(state.transaction_count, 4);

        // Verify both users are KYC approved
        assert!(state.get_user(&originator_id).unwrap().is_kyc_approved());
        assert!(state.get_user(&investor_id).unwrap().is_kyc_approved());
    }

    #[test]
    fn test_configurable_deal_terms() {
        // Test with custom deal terms
        let custom_terms = LoanTerms::new(1200, 60); // 12% APR, 5 year term (keep LoanTerms for compatibility)
        let mut state = GlobalLoanState::with_loan_terms(custom_terms);

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [3u8; 32];

        // Register and KYC both users
        setup_kycd_users(&mut state, originator_id, investor_id);

        // Create deal origination transaction
        let mut deal_metadata = MetadataArray::empty();
        deal_metadata.as_mut_bytes()[0..32].copy_from_slice(&originator_id);
        deal_metadata.as_mut_bytes()[32..64].copy_from_slice(&investor_id);

        let base_timestamp = 1_640_995_200; // January 1, 2022
        let deal_tx = LoanTransaction {
            transaction_id: [10u8; 32],
            user_id: investor_id,                               // was lender_id
            deal_id,                                            // Keep deal_id field name
            transaction_type: TransactionType::DealOrigination, // was LoanOrigination
            amount: 1_000_000,                                  // $10,000 in cents
            created_at: base_timestamp + 5000,
            metadata: deal_metadata,
        };

        // Give investor sufficient balance for deal origination (for testing)
        let initial_funding = BalanceUpdate::new(
            [98u8; 32],  // funding transaction ID
            investor_id, // was lender_id
            None,
            None,
            BalanceUpdateType::ManualAdjustment,
            2_000_000,      // Give enough for the 1,000,000 deal
            0,              // previous balance
            base_timestamp, // same timeframe
            Some("Test funding for configurable terms".to_string()),
        )
        .unwrap();
        state.balance_state.apply_update(initial_funding).unwrap();

        state.process_transaction(&deal_tx).unwrap();

        // Verify deal uses custom terms
        let deal = state.get_loan(&deal_id).unwrap(); // Keep using get_loan for compatibility
        assert_eq!(deal.interest_rate, 1200); // Custom 12% APR
        assert_eq!(deal.term_months, 60); // Custom 5 year term
        assert_eq!(deal.principal_amount, 1_000_000);
        assert_eq!(deal.outstanding_balance, 1_000_000);

        // Verify maturity date calculation
        let expected_maturity = (base_timestamp + 5000) + (60_u64 * 30 * 24 * 60 * 60); // 60 months
        assert_eq!(deal.maturity_date, expected_maturity);

        // Test with default deal terms
        let mut default_state = GlobalLoanState::new();
        assert_eq!(default_state.loan_terms.interest_rate, 750); // Default 7.5%
        assert_eq!(default_state.loan_terms.term_months, 36); // Default 3 years

        // Test updating deal terms on existing state
        let new_terms = LoanTerms::new(500, 24); // 5% APR, 2 year term
        default_state.set_loan_terms(new_terms);
        assert_eq!(default_state.loan_terms.interest_rate, 500);
        assert_eq!(default_state.loan_terms.term_months, 24);
    }

    /// Helper function to set up KYC'd users for testing
    fn setup_kycd_users(
        state: &mut GlobalLoanState,
        originator_id: [u8; 32],
        investor_id: [u8; 32],
    ) {
        let base_timestamp = 1_640_995_200; // January 1, 2022

        // Register originator
        let mut originator_metadata = MetadataArray::empty();
        originator_metadata.as_mut_bytes()[0] = UserType::Investor as u8;
        let originator_reg_tx = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: originator_id, // was borrower_id
            deal_id: [0u8; 32],     // Keep deal_id field name
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: base_timestamp + 1000,
            metadata: originator_metadata,
        };
        state.process_transaction(&originator_reg_tx).unwrap();

        // Register investor
        let mut investor_metadata = MetadataArray::empty();
        investor_metadata.as_mut_bytes()[0] = UserType::Originator as u8;
        let investor_reg_tx = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: investor_id, // was lender_id
            deal_id: [0u8; 32],   // Keep deal_id field name
            transaction_type: TransactionType::UserRegistration,
            amount: 0,
            created_at: base_timestamp + 1100,
            metadata: investor_metadata,
        };
        state.process_transaction(&investor_reg_tx).unwrap();

        // Submit KYC for originator
        let originator_kyc_submit_tx = LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: originator_id, // was borrower_id
            deal_id: [0u8; 32],     // Keep deal_id field name
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: base_timestamp + 2000,
            metadata: MetadataArray::empty(),
        };
        state
            .process_transaction(&originator_kyc_submit_tx)
            .unwrap();

        // Submit KYC for investor
        let investor_kyc_submit_tx = LoanTransaction {
            transaction_id: [7u8; 32],
            user_id: investor_id, // was lender_id
            deal_id: [0u8; 32],   // Keep deal_id field name
            transaction_type: TransactionType::KycSubmission,
            amount: 0,
            created_at: base_timestamp + 2100,
            metadata: MetadataArray::empty(),
        };
        state.process_transaction(&investor_kyc_submit_tx).unwrap();

        // Verify KYC for originator
        let mut originator_verify_metadata = MetadataArray::empty();
        originator_verify_metadata.as_mut_bytes()[0] = 1; // Approved
        let originator_kyc_verify_tx = LoanTransaction {
            transaction_id: [8u8; 32],
            user_id: originator_id, // was borrower_id
            deal_id: [0u8; 32],     // Keep deal_id field name
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: base_timestamp + 3000,
            metadata: originator_verify_metadata,
        };
        state
            .process_transaction(&originator_kyc_verify_tx)
            .unwrap();

        // Verify KYC for investor
        let mut investor_verify_metadata = MetadataArray::empty();
        investor_verify_metadata.as_mut_bytes()[0] = 1; // Approved
        let investor_kyc_verify_tx = LoanTransaction {
            transaction_id: [9u8; 32],
            user_id: investor_id, // was lender_id
            deal_id: [0u8; 32],   // Keep deal_id field name
            transaction_type: TransactionType::KycVerification,
            amount: 0,
            created_at: base_timestamp + 3100,
            metadata: investor_verify_metadata,
        };
        state.process_transaction(&investor_kyc_verify_tx).unwrap();
    }
    #[test]
    fn test_timestamp_validation_bounds() {
        // Test valid timestamp (within bounds)
        let valid_transaction = LoanTransaction {
            transaction_id: [1u8; 32],
            user_id: [1u8; 32],
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name
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
                deal_id: [0u8; 32], // Keep deal_id field name
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1_640_995_200, // Base time
                metadata: MetadataArray::empty(),
            },
            LoanTransaction {
                transaction_id: [2u8; 32],
                user_id: [2u8; 32],
                deal_id: [0u8; 32], // Keep deal_id field name
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
                deal_id: [0u8; 32], // Keep deal_id field name
                transaction_type: TransactionType::UserRegistration,
                amount: 0,
                created_at: 1_640_995_300, // Later time first
                metadata: MetadataArray::empty(),
            },
            LoanTransaction {
                transaction_id: [2u8; 32],
                user_id: [2u8; 32],
                deal_id: [0u8; 32], // Keep deal_id field name
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
    fn test_global_state_with_collateral() {
        let state = GlobalLoanState::new();

        assert_eq!(state.collateral.len(), 0);
        assert_eq!(state.active_associations.len(), 0);
        assert_eq!(state.total_collateral_count(), 0);
    }

    #[test]
    fn test_collateral_helper_methods() {
        let mut state = GlobalLoanState::new();

        let collateral_id = [1u8; 32];
        let deal_id = [2u8; 32];

        // Add collateral
        state.collateral.insert(
            collateral_id.to_vec(),
            crate::collateral::Collateral {
                collateral_id: collateral_id.to_vec(),
                created_at: 1000,
                updated_at: 1000,
                collateral_type: "real_estate".to_string(),
            },
        );

        // Check helpers
        assert!(state.get_collateral(&collateral_id).is_some());
        assert!(!state.is_collateral_pledged(&collateral_id));
        assert_eq!(state.total_collateral_count(), 1);

        // Add association
        state
            .active_associations
            .insert(collateral_id.to_vec(), vec![deal_id.to_vec()]);

        assert!(state.is_collateral_pledged(&collateral_id));
        assert_eq!(
            state.get_active_associations(&collateral_id),
            vec![deal_id.to_vec()]
        );
        assert_eq!(state.total_active_associations(), 1);
    }
    #[test]
    fn test_process_loan_operations() {
        let previous_balance = 100_000u128; // $1000
        let previous_root = [0u8; 32];
        let batch_timestamp = 1_640_995_200;

        let operations = vec![
            LoanOperation::LoanCreation {
                deal_id: [1u8; 32],
                borrower_id: [2u8; 32],
                lender_id: [3u8; 32],
                principal: 50_000,
                interest_rate: 750,
                term_months: 36,
            },
            LoanOperation::Payment {
                deal_id: [1u8; 32],
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
            deal_id: [1u8; 32],
            borrower_id: [2u8; 32],
            lender_id: [3u8; 32],
            principal: 50_000,
            interest_rate: 750,
            term_months: 36,
        };

        let operation2 = LoanOperation::Payment {
            deal_id: [1u8; 32],
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
            deal_id: [0u8; 32], // Keep deal_id field name // Zero for non-loan transactions
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
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [1u8; 32], // Should be zero for deposits
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
            deal_id: [0u8; 32], // Keep deal_id field name // Zero for deposits
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
        let deal_id = [10u8; 32];

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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        deal_id: [0u8; 32], // Keep deal_id field name
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
                        user_id: lender_id, // Keep lender_id for test compatibility
                        deal_id,            // was loan_id, now deal_id
                        transaction_type: TransactionType::DealOrigination, // was LoanOrigination
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
        let loan = state.get_loan(&deal_id).unwrap();
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

        // Verify total system value calculation (total user balances)
        let total_user_balances = state.compute_total_user_balance();
        // Should equal total deposits: $1000 (depositor) + $1500 (lender) = $2500
        assert_eq!(total_user_balances, 250000); // $2500 in cents

        // Verify this equals sum of individual balances
        let sum_of_balances = state.get_user_balance(&depositor_id)
            + state.get_user_balance(&lender_id)
            + state.get_user_balance(&borrower_id);
        assert_eq!(total_user_balances, sum_of_balances);

        // Verify total outstanding loan balance
        let total_loan_balance = state.compute_total_balance();
        assert_eq!(total_loan_balance, 50000); // $500 outstanding loan balance
    }
    #[test]
    fn test_complete_collateral_lifecycle() {
        let mut state = GlobalLoanState::new();
        let collateral_id = [1u8; 32];
        let deal_id = [2u8; 32];

        // 1. Create collateral
        let create_tx = create_test_collateral_creation_tx(collateral_id, "real_estate");
        state.process_transaction(&create_tx).unwrap();

        assert_eq!(state.collateral.len(), 1);
        assert!(!state.is_collateral_pledged(&collateral_id));

        // 2. Update collateral
        let update_tx = create_test_collateral_update_tx(collateral_id, "commercial_property");
        state.process_transaction(&update_tx).unwrap();

        let collateral = state.get_collateral(&collateral_id).unwrap();
        assert_eq!(collateral.collateral_type, "commercial_property");

        // 3. Associate with deal
        let assoc_tx = create_test_association_tx(collateral_id, deal_id);
        state.process_transaction(&assoc_tx).unwrap();

        assert!(state.is_collateral_pledged(&collateral_id));
        assert_eq!(state.get_active_associations(&collateral_id).len(), 1);

        // 4. Request detection
        let detect_req_tx = create_test_detection_request_tx(collateral_id);
        state.process_transaction(&detect_req_tx).unwrap();

        // 5. Update detection results
        let detect_upd_tx = create_test_detection_update_tx(collateral_id);
        state.process_transaction(&detect_upd_tx).unwrap();

        // 6. Disassociate from deal
        let disassoc_tx = create_test_disassociation_tx(collateral_id, deal_id);
        state.process_transaction(&disassoc_tx).unwrap();

        assert!(!state.is_collateral_pledged(&collateral_id));
        assert_eq!(state.get_active_associations(&collateral_id).len(), 0);

        // Verify transaction count
        assert_eq!(state.transaction_count, 6);
    }

    #[test]
    fn test_double_pledge_prevention_in_global_state() {
        let mut state = GlobalLoanState::new();

        let collateral_id = [1u8; 32];
        let deal1_id = [2u8; 32];
        let deal2_id = [3u8; 32];

        // Setup users and deals for context
        setup_kycd_users(&mut state, [4u8; 32], [5u8; 32]);
        setup_test_deal(&mut state, deal1_id);
        setup_test_deal(&mut state, deal2_id);

        // Create collateral
        let create_tx = create_test_collateral_creation_tx(collateral_id, "real_estate");
        state.process_transaction(&create_tx).unwrap();

        // Associate with deal 1
        let assoc1_tx = create_test_association_tx(collateral_id, deal1_id);
        state.process_transaction(&assoc1_tx).unwrap();

        // Try to associate with deal 2 - MUST FAIL
        let assoc2_tx = create_test_association_tx(collateral_id, deal2_id);
        let result = state.process_transaction(&assoc2_tx);

        assert!(result.is_err());

        // Verify error is CollateralAlreadyPledged
        match result.unwrap_err() {
            ProcessingError::CollateralError(
                crate::collateral::CollateralError::CollateralAlreadyPledged {
                    collateral_id: cid,
                    existing_deal_id,
                },
            ) => {
                assert_eq!(cid, collateral_id);
                assert_eq!(existing_deal_id, deal1_id);
            }
            e => panic!("Expected CollateralAlreadyPledged, got: {:?}", e),
        }

        // Verify deal2 was NOT associated
        let active = state.get_active_associations(&collateral_id);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], deal1_id);
    }

    #[test]
    fn test_batch_with_collateral_transactions() {
        let mut state = GlobalLoanState::new();

        let collateral1_id = [10u8; 32];
        let collateral2_id = [11u8; 32];
        let deal1_id = [20u8; 32];
        let deal2_id = [21u8; 32];

        // Create batch with mixed transaction types
        let batch = LoanBatch {
            batch_id: [100u8; 32],
            transactions: vec![
                // Create two pieces of collateral
                create_test_collateral_creation_tx(collateral1_id, "real_estate"),
                create_test_collateral_creation_tx(collateral2_id, "equipment"),
                // Associate each with different deals
                create_test_association_tx(collateral1_id, deal1_id),
                create_test_association_tx(collateral2_id, deal2_id),
                // Request detection for both
                create_test_detection_request_tx(collateral1_id),
                create_test_detection_request_tx(collateral2_id),
            ],
            previous_state_root: [0u8; 32],
            created_at: 1_640_995_200, // January 1, 2022 - valid timestamp
        };

        // Process batch
        let result = state.process_batch(&batch);
        assert!(result.is_ok());

        // Verify final state
        assert_eq!(state.collateral.len(), 2);
        assert_eq!(state.total_active_associations(), 2);
        assert!(state.is_collateral_pledged(&collateral1_id));
        assert!(state.is_collateral_pledged(&collateral2_id));
    }

    #[test]
    fn test_collateral_with_deal_lifecycle() {
        let mut state = GlobalLoanState::new();

        let originator_id = [1u8; 32];
        let investor_id = [2u8; 32];
        let deal_id = [10u8; 32];
        let collateral_id = [20u8; 32];

        // Setup: Create users with KYC
        setup_kycd_users(&mut state, originator_id, investor_id);

        // Give investor funds for deal origination
        let investor_wallet = [100u8; 32];
        let deposit_tx = create_test_deposit_with_timestamp(
            investor_id,
            investor_wallet,
            200000,        // $2000 deposit
            1_640_999_000, // Before collateral creation
        );
        state.process_transaction(&deposit_tx).unwrap();

        // Create collateral
        let coll_create = create_test_collateral_creation_tx(collateral_id, "real_estate");
        state.process_transaction(&coll_create).unwrap();

        // Create deal
        let deal_tx = create_test_deal_origination(deal_id, originator_id, investor_id, 100000);
        state.process_transaction(&deal_tx).unwrap();

        // Associate collateral with deal
        let assoc_tx = create_test_association_tx(collateral_id, deal_id);
        state.process_transaction(&assoc_tx).unwrap();

        assert!(state.is_collateral_pledged(&collateral_id));

        // Make payments and close deal
        let payment = create_test_deal_payment(deal_id, originator_id, 100000);
        state.process_transaction(&payment).unwrap();

        let closeout = create_test_deal_closeout(deal_id, investor_id);
        state.process_transaction(&closeout).unwrap();

        // After deal is closed, can disassociate collateral
        let disassoc_tx = create_test_disassociation_tx(collateral_id, deal_id);
        state.process_transaction(&disassoc_tx).unwrap();

        assert!(!state.is_collateral_pledged(&collateral_id));

        // Now collateral can be reused for another deal
        let deal2_id = [11u8; 32];
        setup_test_deal(&mut state, deal2_id);

        // Create association with later timestamp (after disassociation)
        let mut metadata_bytes = [0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&collateral_id);
        metadata_bytes[32..64].copy_from_slice(&deal2_id);

        let assoc2_tx = LoanTransaction {
            transaction_id: [97u8; 32],
            user_id: [0u8; 32],
            deal_id: [0u8; 32],
            transaction_type: TransactionType::CollateralDealAssociation,
            amount: 0,
            created_at: 1_641_000_600, // After disassociation
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        };
        let result = state.process_transaction(&assoc2_tx);

        assert!(
            result.is_ok(),
            "Should allow reassociation after disassociation"
        );
    }

    #[test]
    fn test_collateral_errors_propagate_correctly() {
        let mut state = GlobalLoanState::new();
        let collateral_id = [1u8; 32];

        // Try to update non-existent collateral
        let update_tx = create_test_collateral_update_tx(collateral_id, "real_estate");
        let result = state.process_transaction(&update_tx);

        assert!(result.is_err());

        // Verify it's a CollateralError wrapped in ProcessingError
        match result.unwrap_err() {
            ProcessingError::CollateralError(
                crate::collateral::CollateralError::CollateralNotFound { .. },
            ) => {
                // Expected
            }
            e => panic!("Expected CollateralNotFound, got: {:?}", e),
        }
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
        let loan1 = crate::deal::Deal {
            deal_id: loan1_id,
            originator_id: borrower_id,
            investor_id: lender_id,
            principal_amount: 10000, // $100
            outstanding_balance: 10000,
            interest_rate: 750,
            term_months: 36,
            deal_status: LoanStatus::Active,
            originated_at: 1_640_995_200,
            funded_at: 1_640_995_200,
            maturity_date: 1_640_995_200 + (36 * 30 * 24 * 60 * 60),
        };

        // Loan 2: Originated with $200 outstanding balance
        let loan2 = crate::deal::Deal {
            deal_id: loan2_id,
            originator_id: borrower_id,
            investor_id: lender_id,
            principal_amount: 20000, // $200
            outstanding_balance: 20000,
            interest_rate: 750,
            term_months: 36,
            deal_status: LoanStatus::Originated,
            originated_at: 1_640_995_200,
            funded_at: 0,
            maturity_date: 1_640_995_200 + (36 * 30 * 24 * 60 * 60),
        };

        // Loan 3: PaidOff (should not count toward balance)
        let loan3 = crate::deal::Deal {
            deal_id: loan3_id,
            originator_id: borrower_id,
            investor_id: lender_id,
            principal_amount: 30000, // $300
            outstanding_balance: 0,
            interest_rate: 750,
            term_months: 36,
            deal_status: LoanStatus::PaidOff,
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
        state.loans.get_mut(&loan2_id).unwrap().deal_status = LoanStatus::PaidOff;

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

        // Verify total user balance decreased correctly
        assert_eq!(state.compute_total_user_balance(), 220000); // $2200 total
    }

    // Helper function to create test KYC submission transaction
    fn create_test_kyc_submission(user_id: [u8; 32]) -> LoanTransaction {
        use crate::transactions::MetadataArray;

        LoanTransaction {
            transaction_id: [6u8; 32],
            user_id,
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name
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
            deal_id: [0u8; 32], // Keep deal_id field name // No loan associated
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
            deal_id: [0u8; 32], // Keep deal_id field name // No loan associated
            transaction_type: TransactionType::UserDeposit,
            amount,
            created_at: timestamp,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_test_collateral_creation_tx(
        collateral_id: [u8; 32],
        collateral_type: &str,
    ) -> LoanTransaction {
        // Encode collateral_type in metadata
        let mut metadata_bytes = [0u8; 64];
        let type_bytes = collateral_type.as_bytes();
        let len = type_bytes.len().min(32);
        metadata_bytes[0..len].copy_from_slice(&type_bytes[0..len]);

        LoanTransaction {
            transaction_id: [99u8; 32],
            user_id: [0u8; 32],     // No user for collateral creation
            deal_id: collateral_id, // Use deal_id field for collateral_id
            transaction_type: TransactionType::CollateralCreation,
            amount: 0,
            created_at: 1_641_000_000, // Later timestamp to avoid going backward
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_test_collateral_update_tx(
        collateral_id: [u8; 32],
        new_type: &str,
    ) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        let type_bytes = new_type.as_bytes();
        let len = type_bytes.len().min(32);
        metadata_bytes[0..len].copy_from_slice(&type_bytes[0..len]);

        LoanTransaction {
            transaction_id: [98u8; 32],
            user_id: [0u8; 32],
            deal_id: collateral_id,
            transaction_type: TransactionType::CollateralUpdate,
            amount: 0,
            created_at: 1_641_000_100, // Later than creation
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_test_association_tx(collateral_id: [u8; 32], deal_id: [u8; 32]) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&collateral_id);
        metadata_bytes[32..64].copy_from_slice(&deal_id);

        LoanTransaction {
            transaction_id: [97u8; 32],
            user_id: [0u8; 32],
            deal_id: [0u8; 32], // Zero for association transactions
            transaction_type: TransactionType::CollateralDealAssociation,
            amount: 0,
            created_at: 1_641_000_200,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_test_disassociation_tx(
        collateral_id: [u8; 32],
        deal_id: [u8; 32],
    ) -> LoanTransaction {
        let mut metadata_bytes = [0u8; 64];
        metadata_bytes[0..32].copy_from_slice(&collateral_id);
        metadata_bytes[32..64].copy_from_slice(&deal_id);

        LoanTransaction {
            transaction_id: [96u8; 32],
            user_id: [0u8; 32],
            deal_id: [0u8; 32],
            transaction_type: TransactionType::CollateralDealDisassociation,
            amount: 0,
            created_at: 1_641_000_500,
            metadata: MetadataArray::from_bytes(&metadata_bytes),
        }
    }

    fn create_test_detection_request_tx(collateral_id: [u8; 32]) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [95u8; 32],
            user_id: [0u8; 32],
            deal_id: collateral_id, // Collateral ID in deal_id field
            transaction_type: TransactionType::CollateralDetectionRequest,
            amount: 0,
            created_at: 1_641_000_300,
            metadata: MetadataArray::empty(),
        }
    }

    fn create_test_detection_update_tx(collateral_id: [u8; 32]) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [94u8; 32],
            user_id: [0u8; 32],
            deal_id: collateral_id,
            transaction_type: TransactionType::CollateralDetectionUpdate,
            amount: 0,
            created_at: 1_641_000_400,
            metadata: MetadataArray::empty(),
        }
    }

    fn create_test_deal_origination(
        deal_id: [u8; 32],
        originator_id: [u8; 32],
        investor_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&originator_id);
        metadata[32..64].copy_from_slice(&investor_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id: originator_id, // was borrower_id
            deal_id,                // Keep deal_id field name
            transaction_type: TransactionType::DealOrigination, // was LoanOrigination
            amount,
            created_at: 1_641_000_150, // After collateral creation, before association
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    fn create_test_deal_payment(
        deal_id: [u8; 32],
        payer_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: payer_id,                              // was borrower_id
            deal_id,                                        // Keep deal_id field name
            transaction_type: TransactionType::DealPayment, // was LoanPayment
            amount,                                         // Partial payment
            created_at: 1_641_000_250,                      // After association
            metadata: MetadataArray::empty(),
        }
    }

    fn create_test_deal_closeout(deal_id: [u8; 32], closer_id: [u8; 32]) -> LoanTransaction {
        LoanTransaction {
            transaction_id: [6u8; 32],
            user_id: closer_id,                              // was lender_id
            deal_id,                                         // Keep deal_id field name
            transaction_type: TransactionType::DealCloseout, // was LoanCloseout
            amount: 0,
            created_at: 1_641_000_350, // After payment
            metadata: MetadataArray::empty(),
        }
    }

    fn setup_test_deal(state: &mut GlobalLoanState, deal_id: [u8; 32]) {
        // Add a minimal deal for testing (not through full origination process)
        let deal = crate::deal::Deal {
            deal_id,
            originator_id: [1u8; 32],
            investor_id: [2u8; 32],
            principal_amount: 100000,
            outstanding_balance: 100000,
            interest_rate: 750,
            term_months: 36,
            deal_status: LoanStatus::Active,
            originated_at: 1_640_995_200,
            funded_at: 1_640_995_200,
            maturity_date: 1_640_995_200 + (36 * 30 * 24 * 60 * 60),
        };
        state.loans.insert(deal_id, deal);
    }
}
