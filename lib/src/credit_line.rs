//! Credit line request and management data structures
//!
//! This module handles credit line requests, evaluations, and active credit line management
//! integrated with the ZK proving system and ADS operations.

use crate::transactions::MetadataArray;
use crate::LiquidityPool;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Credit line request status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CreditLineRequestStatus {
    Submitted = 1,
    Evaluating = 2,
    Approved = 3,
    Rejected = 4,
    Expired = 5,
}

impl CreditLineRequestStatus {
    /// Check if request is in a final state
    #[must_use]
    pub const fn is_final(&self) -> bool {
        matches!(self, Self::Approved | Self::Rejected | Self::Expired)
    }

    /// Check if request can be modified
    #[must_use]
    pub const fn can_modify(&self) -> bool {
        matches!(self, Self::Submitted)
    }
}

/// Active credit line status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CreditLineStatus {
    Active = 1,
    Suspended = 2,
    Closed = 3,
    Expired = 4,
}

impl CreditLineStatus {
    /// Check if credit line is available for drawdown
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Check if credit line is in a final state
    #[must_use]
    pub const fn is_final(&self) -> bool {
        matches!(self, Self::Closed | Self::Expired)
    }
}

/// Credit line request record (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditLineRequest {
    pub request_id: [u8; 32],            // Hashed UUID
    pub merchant_id: [u8; 32],           // Hashed merchant ID
    pub requested_amount: u64,           // Amount in cents
    pub requested_rate_apr: Option<u64>, // Rate in basis points (optional)
    pub requested_tenor_days: u32,       // Default 30 days
    pub collateral_basis: u64,           // Calculated from turnover
    pub ltv_ratio: u64,                  // LTV in basis points (e.g., 8000 = 80%)
    pub status: CreditLineRequestStatus,
    pub matched_pool_id: Option<[u8; 32]>, // Pool that matched
    pub approved_amount: Option<u64>,      // May be less than requested
    pub approved_rate_apr: Option<u64>,    // Approved rate in basis points
    pub rejection_reason: Option<String>,  // Reason for rejection if applicable
    pub created_at: u64,                   // Unix timestamp
    pub updated_at: u64,                   // Unix timestamp
    pub evaluation_notes: MetadataArray,   // JSON-serialized evaluation data
}

impl CreditLineRequest {
    /// Create new credit line request
    #[must_use]
    pub fn new(
        request_id: [u8; 32],
        merchant_id: [u8; 32],
        requested_amount: u64,
        requested_rate_apr: Option<u64>,
        requested_tenor_days: u32,
        created_at: u64,
    ) -> Self {
        Self {
            request_id,
            merchant_id,
            requested_amount,
            requested_rate_apr,
            requested_tenor_days,
            collateral_basis: 0,
            ltv_ratio: 0,
            status: CreditLineRequestStatus::Submitted,
            matched_pool_id: None,
            approved_amount: None,
            approved_rate_apr: None,
            rejection_reason: None,
            created_at,
            updated_at: created_at,
            evaluation_notes: MetadataArray::empty(),
        }
    }

    /// Calculate LTV ratio based on collateral basis
    pub fn calculate_ltv(&mut self) -> Result<(), CreditLineError> {
        if self.collateral_basis == 0 {
            return Err(CreditLineError::InsufficientCollateral);
        }

        // LTV = (requested_amount / collateral_basis) * 10000 (basis points)
        self.ltv_ratio = (self.requested_amount * 10000) / self.collateral_basis;
        Ok(())
    }

    /// Check if request meets basic validation requirements
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.requested_amount > 0
            && self.requested_tenor_days > 0
            && self.requested_tenor_days <= 365
            && self.ltv_ratio <= 20000 // Max 200% LTV
    }
}

/// Active credit line record (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditLine {
    pub credit_line_id: [u8; 32],   // Hashed UUID
    pub request_id: [u8; 32],       // Original request
    pub merchant_id: [u8; 32],      // Merchant who owns this credit line
    pub pool_id: [u8; 32],          // Pool providing the liquidity
    pub credit_limit: u64,          // Maximum credit available
    pub available_credit: u64,      // Currently available for drawdown
    pub utilized_amount: u64,       // Currently outstanding balance
    pub rate_apr: u64,              // APR in basis points
    pub repayment_window_days: u32, // Days until repayment required
    pub status: CreditLineStatus,
    pub opened_at: u64,          // Unix timestamp when opened
    pub expires_at: Option<u64>, // Unix timestamp when expires
    pub closed_at: Option<u64>,  // Unix timestamp when closed
}

impl CreditLine {
    /// Create new active credit line
    #[must_use]
    pub fn new(
        credit_line_id: [u8; 32],
        request_id: [u8; 32],
        merchant_id: [u8; 32],
        pool_id: [u8; 32],
        credit_limit: u64,
        rate_apr: u64,
        repayment_window_days: u32,
        opened_at: u64,
    ) -> Self {
        Self {
            credit_line_id,
            request_id,
            merchant_id,
            pool_id,
            credit_limit,
            available_credit: credit_limit,
            utilized_amount: 0,
            rate_apr,
            repayment_window_days,
            status: CreditLineStatus::Active,
            opened_at,
            expires_at: Some(opened_at + (repayment_window_days as u64 * 86400)), // Convert days to seconds
            closed_at: None,
        }
    }

    /// Calculate utilization percentage (in basis points)
    #[must_use]
    pub fn utilization_percentage(&self) -> u64 {
        if self.credit_limit == 0 {
            return 0;
        }
        (self.utilized_amount * 10000) / self.credit_limit
    }

    /// Check if credit line is available for drawdown
    #[must_use]
    pub fn can_drawdown(&self, amount: u64) -> bool {
        self.status.is_available() && self.available_credit >= amount && amount > 0
    }

    /// Check if credit line is expired
    #[must_use]
    pub fn is_expired(&self, current_time: u64) -> bool {
        if let Some(expires_at) = self.expires_at {
            current_time > expires_at
        } else {
            false
        }
    }

    /// Process drawdown (decreases available credit, increases utilized amount)
    pub fn drawdown(&mut self, amount: u64) -> Result<(), CreditLineError> {
        if !self.can_drawdown(amount) {
            return Err(CreditLineError::InsufficientCredit);
        }

        self.available_credit -= amount;
        self.utilized_amount += amount;
        Ok(())
    }

    /// Process repayment (increases available credit, decreases utilized amount)
    pub fn repay(&mut self, amount: u64) -> Result<(), CreditLineError> {
        if amount == 0 || amount > self.utilized_amount {
            return Err(CreditLineError::InvalidRepaymentAmount);
        }

        self.utilized_amount -= amount;
        self.available_credit += amount;
        Ok(())
    }
}

/// Credit line utilization transaction types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum CreditUtilizationType {
    Drawdown = 1,
    Repayment = 2,
    InterestCharge = 3,
    Fee = 4,
    Reversal = 5,
}

/// Credit line utilization record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditUtilization {
    pub utilization_id: [u8; 32], // Hashed UUID
    pub credit_line_id: [u8; 32], // Credit line being utilized
    pub transaction_type: CreditUtilizationType,
    pub amount: u64,                           // Amount in cents
    pub previous_balance: u64,                 // Balance before this transaction
    pub new_balance: u64,                      // Balance after this transaction
    pub created_at: u64,                       // Unix timestamp
    pub transaction_reference: Option<String>, // External reference
    pub description: Option<String>,           // Human-readable description
}

/// Eligibility evaluation result
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EligibilityResult {
    pub eligible: bool,
    pub checks: HashMap<String, bool>,
    pub score: u8, // 0-100
    pub reasons: Vec<String>,
}

impl EligibilityResult {
    /// Create new eligibility result
    #[must_use]
    pub fn new(eligible: bool, score: u8) -> Self {
        Self {
            eligible,
            checks: HashMap::new(),
            score,
            reasons: Vec::new(),
        }
    }

    /// Add eligibility check result
    pub fn add_check(&mut self, name: String, passed: bool) {
        self.checks.insert(name.clone(), passed);
        if !passed {
            self.reasons.push(format!("Failed check: {}", name));
        }
    }
}

/// Pool matching result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PoolMatchingResult {
    pub collateral_basis: u64,
    pub ltv_ratio: u64,
    pub matching_pools_count: u32,
    pub selected_pool: Option<LiquidityPool>,
    pub approval_recommended: bool,
    pub match_score: u8, // 0-100
}

/// Credit line state management (zkVM compatible)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditLineState {
    /// Active credit line requests by request ID
    pub requests: HashMap<[u8; 32], CreditLineRequest>,
    /// Active credit lines by credit line ID
    pub active_lines: HashMap<[u8; 32], CreditLine>,
    /// Credit utilization history
    pub utilization_history: Vec<CreditUtilization>,
    /// Total credit extended across all lines
    pub total_credit_extended: u64,
    /// Total credit currently outstanding
    pub total_credit_outstanding: u64,
}

impl CreditLineState {
    /// Create new empty credit line state
    #[must_use]
    pub fn new() -> Self {
        Self {
            requests: HashMap::new(),
            active_lines: HashMap::new(),
            utilization_history: Vec::new(),
            total_credit_extended: 0,
            total_credit_outstanding: 0,
        }
    }

    /// Add credit line request
    pub fn add_request(&mut self, request: CreditLineRequest) {
        self.requests.insert(request.request_id, request);
    }

    /// Update request status
    pub fn update_request_status(
        &mut self,
        request_id: [u8; 32],
        status: CreditLineRequestStatus,
    ) -> Result<(), CreditLineError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(CreditLineError::RequestNotFound)?;

        request.status = status;
        request.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(())
    }

    /// Approve credit line request and create active credit line
    pub fn approve_request(
        &mut self,
        request_id: [u8; 32],
        credit_line_id: [u8; 32],
        approved_amount: u64,
        approved_rate: u64,
        pool_id: [u8; 32],
        repayment_window_days: u32,
        current_time: u64,
    ) -> Result<(), CreditLineError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(CreditLineError::RequestNotFound)?;

        // Update request
        request.status = CreditLineRequestStatus::Approved;
        request.approved_amount = Some(approved_amount);
        request.approved_rate_apr = Some(approved_rate);
        request.matched_pool_id = Some(pool_id);
        request.updated_at = current_time;

        // Create active credit line
        let credit_line = CreditLine::new(
            credit_line_id,
            request_id,
            request.merchant_id,
            pool_id,
            approved_amount,
            approved_rate,
            repayment_window_days,
            current_time,
        );

        self.active_lines.insert(credit_line_id, credit_line);
        self.total_credit_extended += approved_amount;
        Ok(())
    }

    /// Reject credit line request
    pub fn reject_request(
        &mut self,
        request_id: [u8; 32],
        reason: String,
        current_time: u64,
    ) -> Result<(), CreditLineError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(CreditLineError::RequestNotFound)?;

        request.status = CreditLineRequestStatus::Rejected;
        request.rejection_reason = Some(reason);
        request.updated_at = current_time;
        Ok(())
    }

    /// Process credit utilization
    pub fn process_utilization(
        &mut self,
        credit_line_id: [u8; 32],
        utilization: CreditUtilization,
    ) -> Result<(), CreditLineError> {
        let credit_line = self
            .active_lines
            .get_mut(&credit_line_id)
            .ok_or(CreditLineError::CreditLineNotFound)?;

        match utilization.transaction_type {
            CreditUtilizationType::Drawdown => {
                credit_line.drawdown(utilization.amount)?;
                self.total_credit_outstanding += utilization.amount;
            }
            CreditUtilizationType::Repayment => {
                credit_line.repay(utilization.amount)?;
                self.total_credit_outstanding -= utilization.amount;
            }
            CreditUtilizationType::InterestCharge
            | CreditUtilizationType::Fee
            | CreditUtilizationType::Reversal => {
                // These don't change the credit line limits but are recorded for history
            }
        }

        self.utilization_history.push(utilization);
        Ok(())
    }

    /// Get credit line by ID
    #[must_use]
    pub fn get_credit_line(&self, credit_line_id: [u8; 32]) -> Option<&CreditLine> {
        self.active_lines.get(&credit_line_id)
    }

    /// Get request by ID
    #[must_use]
    pub fn get_request(&self, request_id: [u8; 32]) -> Option<&CreditLineRequest> {
        self.requests.get(&request_id)
    }

    /// Process credit line drawdown
    pub fn process_drawdown(
        &mut self,
        credit_line_id: [u8; 32],
        amount: u64,
        recipient_address: [u8; 32],
        current_time: u64,
    ) -> Result<(), CreditLineError> {
        let utilization = CreditUtilization {
            utilization_id: credit_line_id, // Use credit line ID as utilization ID for drawdowns
            credit_line_id,
            transaction_type: CreditUtilizationType::Drawdown,
            amount,
            previous_balance: self
                .get_credit_line(credit_line_id)
                .map(|cl| cl.utilized_amount)
                .unwrap_or(0),
            new_balance: self
                .get_credit_line(credit_line_id)
                .map(|cl| cl.utilized_amount + amount)
                .unwrap_or(amount),
            created_at: current_time,
            transaction_reference: Some(format!("0x{}", hex::encode(recipient_address))),
            description: Some(format!("Credit drawdown to recipient")),
        };

        self.process_utilization(credit_line_id, utilization)
    }

    /// Process credit line repayment
    pub fn process_repayment(
        &mut self,
        credit_line_id: [u8; 32],
        amount: u64,
        payment_type: u8,
        current_time: u64,
    ) -> Result<(), CreditLineError> {
        let payment_desc = match payment_type {
            0 => "Principal payment",
            1 => "Interest payment",
            2 => "Combined principal and interest payment",
            _ => "Payment",
        };

        let utilization = CreditUtilization {
            utilization_id: credit_line_id, // Use credit line ID as utilization ID for repayments
            credit_line_id,
            transaction_type: CreditUtilizationType::Repayment,
            amount,
            previous_balance: self
                .get_credit_line(credit_line_id)
                .map(|cl| cl.utilized_amount)
                .unwrap_or(0),
            new_balance: self
                .get_credit_line(credit_line_id)
                .map(|cl| cl.utilized_amount.saturating_sub(amount))
                .unwrap_or(0),
            created_at: current_time,
            transaction_reference: None,
            description: Some(payment_desc.to_string()),
        };

        self.process_utilization(credit_line_id, utilization)
    }
}

impl Default for CreditLineState {
    fn default() -> Self {
        Self::new()
    }
}

/// Credit line specific errors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CreditLineError {
    RequestNotFound,
    CreditLineNotFound,
    InsufficientCollateral,
    InsufficientCredit,
    InvalidRepaymentAmount,
    InvalidLtvRatio,
    InvalidRequest,
    EvaluationFailed(String),
    PoolMatchingFailed,
    ApprovalProcessFailed(String),
}

impl std::fmt::Display for CreditLineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestNotFound => write!(f, "Credit line request not found"),
            Self::CreditLineNotFound => write!(f, "Credit line not found"),
            Self::InsufficientCollateral => write!(f, "Insufficient collateral basis"),
            Self::InsufficientCredit => write!(f, "Insufficient available credit"),
            Self::InvalidRepaymentAmount => write!(f, "Invalid repayment amount"),
            Self::InvalidLtvRatio => write!(f, "Invalid LTV ratio"),
            Self::InvalidRequest => write!(f, "Invalid credit line request"),
            Self::EvaluationFailed(msg) => write!(f, "Credit evaluation failed: {}", msg),
            Self::PoolMatchingFailed => write!(f, "No matching pools found"),
            Self::ApprovalProcessFailed(msg) => write!(f, "Approval process failed: {}", msg),
        }
    }
}

impl std::error::Error for CreditLineError {}
