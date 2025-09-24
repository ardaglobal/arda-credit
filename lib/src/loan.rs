//! Loan management logic for zkVM execution
//!
//! This module contains loan-related types and state transition logic
//! including loan origination, payment processing, closeout operations,
//! and liquidity pool management for auto-deployment.

use crate::transactions::{LoanTransaction, TransactionType};
use crate::user::{User, UserError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// LOAN TERMS - Configurable loan parameters
// ============================================================================

/// Configurable loan terms (zkVM compatible)
///
/// This struct allows you to configure loan terms such as interest rate and term length.
/// It provides sensible defaults but allows customization for different loan products.
///
/// # Examples
///
/// ```rust
/// use lib::{LoanTerms, GlobalLoanState};
///
/// // Use default terms (7.5% APR, 36 months)
/// let default_state = GlobalLoanState::new();
///
/// // Create custom terms (12% APR, 60 months)
/// let custom_terms = LoanTerms::new(1200, 60);
/// let custom_state = GlobalLoanState::with_loan_terms(custom_terms);
///
/// // Update terms on existing state
/// let mut state = GlobalLoanState::new();
/// let new_terms = LoanTerms::new(500, 24); // 5% APR, 2 years
/// state.set_loan_terms(new_terms);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanTerms {
    /// Interest rate in basis points (e.g., 750 = 7.5% APR)
    pub interest_rate: u32,
    /// Loan term in months (e.g., 36 = 3 years)
    pub term_months: u16,
}

impl Default for LoanTerms {
    fn default() -> Self {
        Self {
            interest_rate: 750, // Default 7.5% APR (750 basis points)
            term_months: 36,    // Default 3 year term
        }
    }
}

impl LoanTerms {
    /// Create new loan terms with custom values
    #[must_use]
    pub const fn new(interest_rate: u32, term_months: u16) -> Self {
        Self {
            interest_rate,
            term_months,
        }
    }

    /// Calculate maturity date based on origination time and term
    #[must_use]
    pub const fn calculate_maturity_date(&self, originated_at: u64) -> u64 {
        originated_at + (self.term_months as u64 * 30 * 24 * 60 * 60)
    }
}

/// Loan status throughout lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LoanStatus {
    Originated = 1, // Created, not funded
    Active = 2,     // Funded and active
    PaidOff = 3,    // Fully repaid
    Closed = 4,     // Closed/settled
    Defaulted = 5,  // In default
}

// ============================================================================
// LIQUIDITY POOL TYPES - Pool management for auto-deployment
// ============================================================================

/// Status of a liquidity pool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PoolStatus {
    Active = 1, // Active and accepting deployments
    Paused = 2, // Temporarily paused
    Closed = 3, // Permanently closed
}

impl Default for PoolStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// Auto-deployment configuration for a liquidity pool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoolDeploymentTerms {
    /// Maximum loan-to-value ratio (in basis points, e.g., 8000 = 80%)
    pub max_ltv_ratio: u32,
    /// Maximum interest rate (in basis points, e.g., 1200 = 12%)
    pub max_interest_rate: u32,
    /// Repayment period in days (7-365 allowed)
    pub repayment_period_days: u16,
}

impl PoolDeploymentTerms {
    /// Create new deployment terms with validation
    ///
    /// # Errors
    /// Returns `PoolError::InvalidParameters` if parameters are out of valid ranges
    pub fn new(
        max_ltv_ratio: u32,
        max_interest_rate: u32,
        repayment_period_days: u16,
    ) -> Result<Self, PoolError> {
        Self::validate_parameters(max_ltv_ratio, max_interest_rate, repayment_period_days)?;

        Ok(Self {
            max_ltv_ratio,
            max_interest_rate,
            repayment_period_days,
        })
    }

    /// Validate pool parameters
    fn validate_parameters(
        max_ltv_ratio: u32,
        max_interest_rate: u32,
        repayment_period_days: u16,
    ) -> Result<(), PoolError> {
        if !(1000..=9500).contains(&max_ltv_ratio) {
            return Err(PoolError::InvalidParameters {
                reason: "LTV ratio must be between 10% (1000) and 95% (9500) basis points"
                    .to_string(),
            });
        }

        if !(100..=50000).contains(&max_interest_rate) {
            return Err(PoolError::InvalidParameters {
                reason: "Interest rate must be between 1% (100) and 50% (50000) basis points"
                    .to_string(),
            });
        }

        if !(7..=365).contains(&repayment_period_days) {
            return Err(PoolError::InvalidParameters {
                reason: "Repayment period must be between 7 and 365 days".to_string(),
            });
        }

        Ok(())
    }
}

impl Default for PoolDeploymentTerms {
    fn default() -> Self {
        Self {
            max_ltv_ratio: 8000,       // 80% default LTV
            max_interest_rate: 1200,   // 12% default max rate
            repayment_period_days: 30, // 30 days default
        }
    }
}

/// Liquidity pool for investor-funded auto-deployment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiquidityPool {
    pub pool_id: [u8; 32],     // Hashed UUID
    pub investor_id: [u8; 32], // Hashed investor UUID
    pub name: String,          // Pool name (max 255 chars)

    // Pool balances
    pub total_deposits: u64,      // Total funds deposited (in cents)
    pub available_liquidity: u64, // Available for new deployments
    pub utilized_liquidity: u64,  // Currently deployed in active loans

    // Auto-deployment terms
    pub deployment_terms: PoolDeploymentTerms,

    pub status: PoolStatus,
    pub created_at: u64, // Unix timestamp
    pub updated_at: u64, // Unix timestamp
}

impl LiquidityPool {
    /// Create a new liquidity pool
    pub fn new(
        pool_id: [u8; 32],
        investor_id: [u8; 32],
        name: String,
        deployment_terms: PoolDeploymentTerms,
        created_at: u64,
    ) -> Self {
        Self {
            pool_id,
            investor_id,
            name,
            total_deposits: 0,
            available_liquidity: 0,
            utilized_liquidity: 0,
            deployment_terms,
            status: PoolStatus::Active,
            created_at,
            updated_at: created_at,
        }
    }

    /// Check if pool can deploy the requested amount
    #[must_use]
    pub fn can_deploy(&self, amount: u64) -> bool {
        self.status == PoolStatus::Active && self.available_liquidity >= amount
    }

    /// Check if pool meets deployment criteria for a loan request
    #[must_use]
    pub fn matches_criteria(
        &self,
        required_amount: u64,
        max_ltv: u32,
        max_rate: u32,
        min_repayment_days: u16,
    ) -> bool {
        self.can_deploy(required_amount)
            && self.deployment_terms.max_ltv_ratio >= max_ltv
            && self.deployment_terms.max_interest_rate >= max_rate
            && self.deployment_terms.repayment_period_days >= min_repayment_days
    }

    /// Deploy liquidity from pool (reduce available, increase utilized)
    pub fn deploy_liquidity(&mut self, amount: u64) -> Result<(), PoolError> {
        if !self.can_deploy(amount) {
            return Err(PoolError::InsufficientLiquidity {
                requested: amount,
                available: self.available_liquidity,
            });
        }

        self.available_liquidity -= amount;
        self.utilized_liquidity += amount;
        Ok(())
    }

    /// Return liquidity to pool (increase available, decrease utilized)  
    pub fn return_liquidity(&mut self, amount: u64) -> Result<(), PoolError> {
        if amount > self.utilized_liquidity {
            return Err(PoolError::InvalidLiquidityReturn {
                amount,
                utilized: self.utilized_liquidity,
            });
        }

        self.utilized_liquidity -= amount;
        self.available_liquidity += amount;
        Ok(())
    }

    /// Validate pool balance consistency
    #[must_use]
    pub fn is_balance_consistent(&self) -> bool {
        self.available_liquidity + self.utilized_liquidity == self.total_deposits
    }
}

/// Loan record (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Loan {
    pub loan_id: [u8; 32],        // Hashed UUID
    pub borrower_id: [u8; 32],    // Hashed user ID
    pub lender_id: [u8; 32],      // Hashed user ID
    pub principal_amount: u64,    // Amount in smallest currency unit (cents)
    pub outstanding_balance: u64, // Current balance owed
    pub interest_rate: u32,       // Rate in basis points (7.5% = 750)
    pub term_months: u16,         // Loan term
    pub loan_status: LoanStatus,
    pub originated_at: u64, // Unix timestamp
    pub funded_at: u64,     // Unix timestamp, 0 if not funded
    pub maturity_date: u64, // Unix timestamp
}

impl Loan {
    /// Check if loan can accept payments
    #[must_use]
    pub const fn can_accept_payment(&self) -> bool {
        matches!(
            self.loan_status,
            LoanStatus::Active | LoanStatus::Originated
        )
    }

    /// Check if loan is fully paid off
    #[must_use]
    pub const fn is_paid_off(&self) -> bool {
        self.outstanding_balance == 0
    }
}

/// Errors specific to loan operations
#[derive(Error, Debug, Clone)]
pub enum LoanError {
    #[error("Loan not found: {loan_id:?}")]
    LoanNotFound { loan_id: [u8; 32] },

    #[error("Invalid loan state transition: {current_status:?} -> {requested_operation:?}")]
    InvalidLoanStateTransition {
        current_status: LoanStatus,
        requested_operation: TransactionType,
    },

    #[error("Insufficient loan balance: requested {amount}, available {balance}")]
    InsufficientLoanBalance { amount: u64, balance: u64 },

    #[error("Duplicate loan: {loan_id:?}")]
    DuplicateLoan { loan_id: [u8; 32] },

    #[error("Invalid transaction type for loan operation")]
    InvalidTransactionType,

    #[error("User error: {0}")]
    UserError(#[from] UserError),

    #[error("Pool error: {0}")]
    PoolError(#[from] PoolError),
}

/// Errors specific to liquidity pool operations
#[derive(Error, Debug, Clone)]
pub enum PoolError {
    #[error("Pool not found: {pool_id:?}")]
    PoolNotFound { pool_id: [u8; 32] },

    #[error("Insufficient liquidity: requested {requested}, available {available}")]
    InsufficientLiquidity { requested: u64, available: u64 },

    #[error("Invalid liquidity return: amount {amount}, utilized {utilized}")]
    InvalidLiquidityReturn { amount: u64, utilized: u64 },

    #[error("Duplicate pool: {pool_id:?}")]
    DuplicatePool { pool_id: [u8; 32] },

    #[error("Invalid pool parameters: {reason}")]
    InvalidParameters { reason: String },

    #[error("Pool is not active: {status:?}")]
    PoolNotActive { status: PoolStatus },

    #[error(
        "Unauthorized pool operation: investor {investor_id:?} cannot access pool {pool_id:?}"
    )]
    UnauthorizedAccess {
        investor_id: [u8; 32],
        pool_id: [u8; 32],
    },

    #[error("Pool name already exists for investor")]
    DuplicatePoolName,

    #[error("User error: {0}")]
    UserError(#[from] UserError),
}

/// Loan state management
pub struct LoanState {
    pub loans: HashMap<[u8; 32], Loan>,
}

/// Liquidity pool state management
pub struct PoolState {
    pub pools: HashMap<[u8; 32], LiquidityPool>,
}

impl PoolState {
    /// Create a new empty pool state
    #[must_use]
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Create a new liquidity pool
    ///
    /// # Errors
    /// Returns various `PoolError` variants for invalid parameters, duplicate pools, or unauthorized access
    pub fn create_pool(
        &mut self,
        pool_id: [u8; 32],
        investor_id: [u8; 32],
        name: String,
        deployment_terms: PoolDeploymentTerms,
        created_at: u64,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), PoolError> {
        // Check for duplicate pool ID
        if self.pools.contains_key(&pool_id) {
            return Err(PoolError::DuplicatePool { pool_id });
        }

        // Verify investor exists and is KYC approved
        let investor = users.get(&investor_id).ok_or(UserError::UserNotFound {
            user_id: investor_id,
        })?;

        if !investor.is_kyc_approved() {
            return Err(PoolError::UserError(UserError::KycNotApproved {
                user_id: investor_id,
            }));
        }

        // Check for duplicate pool name for this investor
        if self.pools.values().any(|pool| {
            pool.investor_id == investor_id
                && pool.name == name
                && pool.status != PoolStatus::Closed
        }) {
            return Err(PoolError::DuplicatePoolName);
        }

        // Create new pool
        let pool = LiquidityPool::new(pool_id, investor_id, name, deployment_terms, created_at);
        self.pools.insert(pool_id, pool);

        Ok(())
    }

    /// Get pool by ID (read-only)
    #[must_use]
    pub fn get_pool(&self, pool_id: &[u8; 32]) -> Option<&LiquidityPool> {
        self.pools.get(pool_id)
    }

    /// Get mutable pool by ID with ownership validation
    ///
    /// # Errors
    /// Returns `PoolError` variants for pool not found or unauthorized access
    pub fn get_pool_mut(
        &mut self,
        pool_id: &[u8; 32],
        investor_id: &[u8; 32],
    ) -> Result<&mut LiquidityPool, PoolError> {
        let pool = self
            .pools
            .get_mut(pool_id)
            .ok_or(PoolError::PoolNotFound { pool_id: *pool_id })?;

        if pool.investor_id != *investor_id {
            return Err(PoolError::UnauthorizedAccess {
                investor_id: *investor_id,
                pool_id: *pool_id,
            });
        }

        Ok(pool)
    }

    /// Get all pools for a specific investor
    #[must_use]
    pub fn get_investor_pools(&self, investor_id: &[u8; 32]) -> Vec<&LiquidityPool> {
        self.pools
            .values()
            .filter(|pool| pool.investor_id == *investor_id)
            .collect()
    }

    /// Find matching pools for auto-deployment
    #[must_use]
    pub fn find_matching_pools(
        &self,
        required_amount: u64,
        max_ltv: u32,
        max_rate: u32,
        min_repayment_days: u16,
    ) -> Vec<&LiquidityPool> {
        let mut matching: Vec<&LiquidityPool> = self
            .pools
            .values()
            .filter(|pool| {
                pool.matches_criteria(required_amount, max_ltv, max_rate, min_repayment_days)
            })
            .collect();

        // Sort by preference: lower interest rates first, then higher liquidity
        matching.sort_by(|a, b| {
            a.deployment_terms
                .max_interest_rate
                .cmp(&b.deployment_terms.max_interest_rate)
                .then_with(|| b.available_liquidity.cmp(&a.available_liquidity))
        });

        matching
    }

    /// Update pool status
    ///
    /// # Errors
    /// Returns `PoolError` variants for pool not found or unauthorized access
    pub fn update_pool_status(
        &mut self,
        pool_id: &[u8; 32],
        investor_id: &[u8; 32],
        new_status: PoolStatus,
    ) -> Result<(), PoolError> {
        let pool = self.get_pool_mut(pool_id, investor_id)?;
        pool.status = new_status;
        Ok(())
    }
}

impl Default for PoolState {
    fn default() -> Self {
        Self::new()
    }
}

impl LoanState {
    /// Create a new empty loan state
    #[must_use]
    pub fn new() -> Self {
        Self {
            loans: HashMap::new(),
        }
    }

    /// Process loan origination transaction
    ///
    /// # Errors
    /// Returns various `LoanError` variants for invalid transaction type, duplicate loans, invalid users, or KYC failures
    pub fn process_loan_origination(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
        loan_terms: &LoanTerms,
    ) -> Result<(), LoanError> {
        if transaction.transaction_type != TransactionType::LoanOrigination {
            return Err(LoanError::InvalidTransactionType);
        }

        let loan_id = transaction.loan_id;
        // Note: initiator_id not currently used in loan origination logic

        // Check for duplicate loan
        if self.loans.contains_key(&loan_id) {
            return Err(LoanError::DuplicateLoan { loan_id });
        }

        // Decode loan participants from metadata (borrower_id and lender_id)
        let metadata = transaction.metadata.as_bytes();
        let mut borrower_id = [0u8; 32];
        let mut lender_id = [0u8; 32];
        borrower_id.copy_from_slice(&metadata[0..32]);
        lender_id.copy_from_slice(&metadata[32..64]);

        // Verify both parties are KYC'd
        let borrower = users.get(&borrower_id).ok_or(UserError::UserNotFound {
            user_id: borrower_id,
        })?;
        let lender = users
            .get(&lender_id)
            .ok_or(UserError::UserNotFound { user_id: lender_id })?;

        if !borrower.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved {
                user_id: borrower_id,
            }));
        }
        if !lender.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved {
                user_id: lender_id,
            }));
        }

        // Create new loan
        let loan = Loan {
            loan_id,
            borrower_id,
            lender_id,
            principal_amount: transaction.amount,
            outstanding_balance: transaction.amount,
            interest_rate: loan_terms.interest_rate,
            term_months: loan_terms.term_months,
            loan_status: LoanStatus::Originated,
            originated_at: transaction.created_at,
            funded_at: 0,
            maturity_date: loan_terms.calculate_maturity_date(transaction.created_at),
        };

        self.loans.insert(loan_id, loan);
        Ok(())
    }

    /// Process loan payment transaction
    ///
    /// # Errors
    /// Returns various `LoanError` variants for invalid transaction type, loan not found, or payment validation failures
    pub fn process_loan_payment(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), LoanError> {
        if transaction.transaction_type != TransactionType::LoanPayment {
            return Err(LoanError::InvalidTransactionType);
        }

        let loan_id = transaction.loan_id;
        let payer_id = transaction.user_id;

        // Verify payer is KYC approved
        let payer = users
            .get(&payer_id)
            .ok_or(UserError::UserNotFound { user_id: payer_id })?;
        if !payer.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved {
                user_id: payer_id,
            }));
        }

        let loan = self
            .loans
            .get_mut(&loan_id)
            .ok_or(LoanError::LoanNotFound { loan_id })?;

        // Verify loan can accept payments
        if !loan.can_accept_payment() {
            return Err(LoanError::InvalidLoanStateTransition {
                current_status: loan.loan_status,
                requested_operation: TransactionType::LoanPayment,
            });
        }

        let payment_amount = transaction.amount;

        // Check if payment exceeds outstanding balance
        if payment_amount > loan.outstanding_balance {
            return Err(LoanError::InsufficientLoanBalance {
                amount: payment_amount,
                balance: loan.outstanding_balance,
            });
        }

        // Apply payment
        loan.outstanding_balance -= payment_amount;

        // Update loan status based on remaining balance
        if loan.outstanding_balance == 0 {
            loan.loan_status = LoanStatus::PaidOff;
        } else if loan.loan_status == LoanStatus::Originated {
            // First payment activates the loan
            loan.loan_status = LoanStatus::Active;
            loan.funded_at = transaction.created_at;
        }

        Ok(())
    }

    /// Process loan closeout transaction
    ///
    /// # Errors
    /// Returns various `LoanError` variants for invalid transaction type, loan not found, or closeout validation failures
    pub fn process_loan_closeout(
        &mut self,
        transaction: &LoanTransaction,
        users: &HashMap<[u8; 32], User>,
    ) -> Result<(), LoanError> {
        if transaction.transaction_type != TransactionType::LoanCloseout {
            return Err(LoanError::InvalidTransactionType);
        }

        let loan_id = transaction.loan_id;
        let closer_id = transaction.user_id;

        // Verify closer is KYC approved
        let closer = users
            .get(&closer_id)
            .ok_or(UserError::UserNotFound { user_id: closer_id })?;
        if !closer.is_kyc_approved() {
            return Err(LoanError::UserError(UserError::KycNotApproved {
                user_id: closer_id,
            }));
        }

        let loan = self
            .loans
            .get_mut(&loan_id)
            .ok_or(LoanError::LoanNotFound { loan_id })?;

        // Only allow closeout of paid off loans
        match loan.loan_status {
            LoanStatus::PaidOff => {
                loan.loan_status = LoanStatus::Closed;
                Ok(())
            }
            _ => Err(LoanError::InvalidLoanStateTransition {
                current_status: loan.loan_status,
                requested_operation: TransactionType::LoanCloseout,
            }),
        }
    }

    /// Get loan by ID (read-only)
    #[must_use]
    pub fn get_loan(&self, loan_id: &[u8; 32]) -> Option<&Loan> {
        self.loans.get(loan_id)
    }
}

impl Default for LoanState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transactions::MetadataArray;
    use crate::user::{KycStatus, UserType};

    fn setup_test_users() -> (HashMap<[u8; 32], User>, [u8; 32], [u8; 32]) {
        let mut users = HashMap::new();
        let borrower_id = [1u8; 32];
        let lender_id = [2u8; 32];

        let borrower = User {
            user_id: borrower_id,
            user_type: UserType::Investor,
            kyc_status: KycStatus::Approved,
            kyc_submitted_at: 1_234_567_890,
            kyc_verified_at: 1_234_567_900,
            created_at: 1_234_567_880,
        };

        let lender = User {
            user_id: lender_id,
            user_type: UserType::Originator,
            kyc_status: KycStatus::Approved,
            kyc_submitted_at: 1_234_567_890,
            kyc_verified_at: 1_234_567_900,
            created_at: 1_234_567_880,
        };

        users.insert(borrower_id, borrower);
        users.insert(lender_id, lender);

        (users, borrower_id, lender_id)
    }

    fn create_loan_origination_transaction(
        loan_id: [u8; 32],
        user_id: [u8; 32],
        borrower_id: [u8; 32],
        lender_id: [u8; 32],
        amount: u64,
    ) -> LoanTransaction {
        let mut metadata = vec![0u8; 64];
        metadata[0..32].copy_from_slice(&borrower_id);
        metadata[32..64].copy_from_slice(&lender_id);

        LoanTransaction {
            transaction_id: [3u8; 32],
            user_id,
            loan_id,
            transaction_type: TransactionType::LoanOrigination,
            amount,
            created_at: 1_234_567_920,
            metadata: MetadataArray::from_bytes(&metadata),
        }
    }

    #[test]
    fn test_loan_origination() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        let transaction = create_loan_origination_transaction(
            loan_id,
            borrower_id, // Borrower initiates
            borrower_id,
            lender_id,
            10000, // $100.00
        );

        let loan_terms = LoanTerms::default();
        let result = loan_state.process_loan_origination(&transaction, &users, &loan_terms);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.principal_amount, 10000);
        assert_eq!(loan.outstanding_balance, 10000);
        assert_eq!(loan.loan_status, LoanStatus::Originated);
    }

    #[test]
    fn test_loan_payment() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // First originate loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        let loan_terms = LoanTerms::default();
        loan_state
            .process_loan_origination(&origination, &users, &loan_terms)
            .unwrap();

        // Then make payment
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 5000, // $50.00 payment
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_payment(&payment_transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.outstanding_balance, 5000);
        assert_eq!(loan.loan_status, LoanStatus::Active); // First payment activates loan
    }

    #[test]
    fn test_loan_full_payoff() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Originate loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        let loan_terms = LoanTerms::default();
        loan_state
            .process_loan_origination(&origination, &users, &loan_terms)
            .unwrap();

        // Full payment
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 10000, // Full payment
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_payment(&payment_transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.outstanding_balance, 0);
        assert_eq!(loan.loan_status, LoanStatus::PaidOff);
        assert!(loan.is_paid_off());
    }

    #[test]
    fn test_loan_closeout() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Setup: Originate and fully pay off loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        let loan_terms = LoanTerms::default();
        loan_state
            .process_loan_origination(&origination, &users, &loan_terms)
            .unwrap();

        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 10000, // Full payment
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };
        loan_state
            .process_loan_payment(&payment_transaction, &users)
            .unwrap();

        // Now closeout
        let closeout_transaction = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanCloseout,
            amount: 0,
            created_at: 1_234_567_940,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_closeout(&closeout_transaction, &users);
        assert!(result.is_ok());

        let loan = loan_state.get_loan(&loan_id).unwrap();
        assert_eq!(loan.loan_status, LoanStatus::Closed);
    }

    #[test]
    fn test_loan_closeout_without_payoff_fails() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Setup: Originate loan but don't pay it off
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        let loan_terms = LoanTerms::default();
        loan_state
            .process_loan_origination(&origination, &users, &loan_terms)
            .unwrap();

        // Try to closeout without paying off
        let closeout_transaction = LoanTransaction {
            transaction_id: [5u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanCloseout,
            amount: 0,
            created_at: 1_234_567_940,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_closeout(&closeout_transaction, &users);
        assert!(matches!(
            result,
            Err(LoanError::InvalidLoanStateTransition { .. })
        ));
    }

    #[test]
    fn test_overpayment_fails() {
        let mut loan_state = LoanState::new();
        let (users, borrower_id, lender_id) = setup_test_users();
        let loan_id = [10u8; 32];

        // Originate loan
        let origination = create_loan_origination_transaction(
            loan_id,
            borrower_id,
            borrower_id,
            lender_id,
            10000,
        );
        let loan_terms = LoanTerms::default();
        loan_state
            .process_loan_origination(&origination, &users, &loan_terms)
            .unwrap();

        // Try to overpay
        let payment_transaction = LoanTransaction {
            transaction_id: [4u8; 32],
            user_id: borrower_id,
            loan_id,
            transaction_type: TransactionType::LoanPayment,
            amount: 15000, // More than outstanding balance
            created_at: 1_234_567_930,
            metadata: MetadataArray::empty(),
        };

        let result = loan_state.process_loan_payment(&payment_transaction, &users);
        assert!(matches!(
            result,
            Err(LoanError::InsufficientLoanBalance { .. })
        ));
    }
}
