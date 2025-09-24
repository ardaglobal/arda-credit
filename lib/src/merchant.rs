//! Merchant state management for zkVM operations
//!
//! Handles merchant profile management, turnover tracking, and document verification
//! within the zero-knowledge context for privacy-preserving merchant operations.

use crate::transactions::{LoanTransaction, TransactionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors related to merchant operations
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum MerchantError {
    #[error("Invalid transaction type for merchant operation")]
    InvalidTransactionType,

    #[error("Merchant not found: {merchant_id:?}")]
    MerchantNotFound { merchant_id: [u8; 32] },

    #[error("Merchant not KYC approved: {merchant_id:?}")]
    MerchantNotKycApproved { merchant_id: [u8; 32] },

    #[error("Invalid merchant metadata format")]
    InvalidMetadata,

    #[error("Turnover submission validation failed")]
    TurnoverValidationFailed,

    #[error("Document verification failed")]
    DocumentVerificationFailed,
}

/// Merchant record in ZK state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantRecord {
    /// Unique merchant identifier
    pub merchant_id: [u8; 32],

    /// Merchant Category Code (MCC) for business type classification
    pub mcc: u32,

    /// KYC status (0 = not submitted, 1 = pending, 2 = approved, 3 = rejected)
    pub kyc_status: u8,

    /// Profile completion status
    pub profile_completed: bool,

    /// Collateral basis for LTV calculations (in wei)
    pub collateral_basis: u64,

    /// Timestamp of last turnover update
    pub last_turnover_update: u64,

    /// Number of verified documents
    pub verified_documents: u32,

    /// Credit line eligibility score (0-1000)
    pub credit_score: u16,
}

/// Turnover summary for merchant over rolling 30-day period
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnoverSummary {
    /// Merchant identifier
    pub merchant_id: [u8; 32],

    /// Total gross receipts over 30 days (in wei)
    pub total_gross_receipts: u64,

    /// Total refunds over 30 days (in wei)
    pub total_refunds: u64,

    /// Net receipts (gross - refunds)
    pub net_receipts: u64,

    /// Number of turnover submissions in period
    pub submission_count: u32,

    /// Timestamp of last update
    pub last_updated: u64,

    /// Verification status percentage (0-100)
    pub verification_rate: u8,
}

/// Merchant state management for zkVM operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantState {
    /// All merchant records indexed by merchant_id
    pub merchants: HashMap<[u8; 32], MerchantRecord>,

    /// 30-day rolling turnover summaries indexed by merchant_id
    pub turnover_summaries: HashMap<[u8; 32], TurnoverSummary>,
}

impl MerchantState {
    /// Create a new empty merchant state
    #[must_use]
    pub fn new() -> Self {
        Self {
            merchants: HashMap::new(),
            turnover_summaries: HashMap::new(),
        }
    }

    /// Process merchant registration transaction
    ///
    /// # Errors
    /// Returns `MerchantError` if transaction type is invalid or merchant already exists
    pub fn process_merchant_registration(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), MerchantError> {
        if transaction.transaction_type != TransactionType::AdminRegistration {
            return Err(MerchantError::InvalidTransactionType);
        }

        let merchant_id = transaction.user_id;

        // Check if merchant already exists
        if self.merchants.contains_key(&merchant_id) {
            return Ok(()); // Registration is idempotent
        }

        // Extract MCC from metadata (first 4 bytes)
        let metadata_bytes = transaction.metadata.as_bytes();
        let mut mcc_bytes = [0u8; 4];
        mcc_bytes.copy_from_slice(&metadata_bytes[0..4]);
        let mcc = u32::from_le_bytes(mcc_bytes);

        // Create new merchant record
        let merchant = MerchantRecord {
            merchant_id,
            mcc,
            kyc_status: 0, // Not submitted
            profile_completed: false,
            collateral_basis: 0,
            last_turnover_update: 0,
            verified_documents: 0,
            credit_score: 0,
        };

        self.merchants.insert(merchant_id, merchant);

        // Initialize empty turnover summary
        let turnover_summary = TurnoverSummary {
            merchant_id,
            total_gross_receipts: 0,
            total_refunds: 0,
            net_receipts: 0,
            submission_count: 0,
            last_updated: transaction.created_at,
            verification_rate: 0,
        };

        self.turnover_summaries
            .insert(merchant_id, turnover_summary);

        Ok(())
    }

    /// Process merchant profile update transaction
    ///
    /// # Errors
    /// Returns `MerchantError` if merchant not found or not KYC approved
    pub fn process_profile_update(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), MerchantError> {
        if transaction.transaction_type != TransactionType::MerchantProfileUpdate {
            return Err(MerchantError::InvalidTransactionType);
        }

        let merchant_id = transaction.user_id;

        let merchant = self
            .merchants
            .get_mut(&merchant_id)
            .ok_or(MerchantError::MerchantNotFound { merchant_id })?;

        let metadata_bytes = transaction.metadata.as_bytes();

        // Update profile completion status (metadata byte 0)
        if metadata_bytes[0] == 1 {
            merchant.profile_completed = true;
        }

        // Update credit score if provided (bytes 1-2)
        let mut score_bytes = [0u8; 2];
        score_bytes.copy_from_slice(&metadata_bytes[1..3]);
        let new_score = u16::from_le_bytes(score_bytes);
        if new_score > 0 {
            merchant.credit_score = new_score;
        }

        Ok(())
    }

    /// Process turnover submission transaction
    ///
    /// # Errors
    /// Returns `MerchantError` if merchant not found or validation fails
    pub fn process_turnover_submission(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), MerchantError> {
        if transaction.transaction_type != TransactionType::SubmitTurnover {
            return Err(MerchantError::InvalidTransactionType);
        }

        let merchant_id = transaction.user_id;

        // Verify merchant exists and is KYC approved
        let merchant = self
            .merchants
            .get_mut(&merchant_id)
            .ok_or(MerchantError::MerchantNotFound { merchant_id })?;

        if merchant.kyc_status != 2 {
            return Err(MerchantError::MerchantNotKycApproved { merchant_id });
        }

        // Extract turnover data from transaction
        let gross_receipts = transaction.amount; // Amount field contains gross receipts

        let metadata_bytes = transaction.metadata.as_bytes();

        // Extract refunds from metadata (first 8 bytes)
        let mut refund_bytes = [0u8; 8];
        refund_bytes.copy_from_slice(&metadata_bytes[0..8]);
        let refunds = u64::from_le_bytes(refund_bytes);

        // Calculate net receipts
        let net_receipts = gross_receipts.saturating_sub(refunds);

        // Update merchant's last turnover timestamp
        merchant.last_turnover_update = transaction.created_at;

        // Update turnover summary
        let summary = self
            .turnover_summaries
            .get_mut(&merchant_id)
            .ok_or(MerchantError::MerchantNotFound { merchant_id })?;

        // Rolling window update - simplified for zkVM compatibility
        summary.total_gross_receipts = summary.total_gross_receipts.saturating_add(gross_receipts);
        summary.total_refunds = summary.total_refunds.saturating_add(refunds);
        summary.net_receipts = summary.net_receipts.saturating_add(net_receipts);
        summary.submission_count = summary.submission_count.saturating_add(1);
        summary.last_updated = transaction.created_at;

        // Update verification rate (metadata byte 8: 0 = unverified, 1 = verified)
        if metadata_bytes[8] == 1 {
            let verified_count =
                (summary.submission_count * summary.verification_rate as u32) / 100 + 1;
            summary.verification_rate = ((verified_count * 100) / summary.submission_count) as u8;
        }

        Ok(())
    }

    /// Process document verification transaction
    ///
    /// # Errors
    /// Returns `MerchantError` if merchant not found or verification fails
    pub fn process_document_verification(
        &mut self,
        transaction: &LoanTransaction,
    ) -> Result<(), MerchantError> {
        if transaction.transaction_type != TransactionType::VerifyDocument {
            return Err(MerchantError::InvalidTransactionType);
        }

        let merchant_id = transaction.user_id;

        let merchant = self
            .merchants
            .get_mut(&merchant_id)
            .ok_or(MerchantError::MerchantNotFound { merchant_id })?;

        let metadata_bytes = transaction.metadata.as_bytes();

        // Verification result in metadata byte 0 (0 = failed, 1 = passed)
        if metadata_bytes[0] == 1 {
            merchant.verified_documents = merchant.verified_documents.saturating_add(1);

            // Update credit score based on document verification
            let score_increase = match merchant.verified_documents {
                1 => 50,      // First document
                2 => 75,      // Second document
                3 => 100,     // Third document
                4..=10 => 25, // Additional documents
                _ => 10,      // Diminishing returns
            };

            merchant.credit_score = merchant
                .credit_score
                .saturating_add(score_increase)
                .min(1000);
        }

        Ok(())
    }

    /// Get merchant record by ID
    pub fn get_merchant(&self, merchant_id: &[u8; 32]) -> Option<&MerchantRecord> {
        self.merchants.get(merchant_id)
    }

    /// Get turnover summary by merchant ID
    pub fn get_turnover_summary(&self, merchant_id: &[u8; 32]) -> Option<&TurnoverSummary> {
        self.turnover_summaries.get(merchant_id)
    }

    /// Calculate LTV eligibility based on turnover and collateral
    pub fn calculate_ltv_eligibility(&self, merchant_id: &[u8; 32]) -> Option<f64> {
        let merchant = self.merchants.get(merchant_id)?;
        let summary = self.turnover_summaries.get(merchant_id)?;

        if merchant.collateral_basis == 0 || summary.net_receipts == 0 {
            return Some(0.0);
        }

        // Base LTV calculation: (30-day net receipts * 12) / collateral_basis * risk_factor
        let annual_turnover = summary.net_receipts * 12;
        let risk_factor = match merchant.credit_score {
            800..=1000 => 0.85, // High credit
            600..=799 => 0.70,  // Medium credit
            400..=599 => 0.55,  // Low credit
            _ => 0.30,          // Very low credit
        };

        let ltv_ratio = (annual_turnover as f64) / (merchant.collateral_basis as f64) * risk_factor;
        Some(ltv_ratio.min(0.90)) // Cap at 90% LTV
    }
}

impl Default for MerchantState {
    fn default() -> Self {
        Self::new()
    }
}
