//! Credit line eligibility validation and LTV calculation logic
//!
//! This module implements the business logic for evaluating merchant eligibility,
//! calculating loan-to-value ratios, and matching credit requests to liquidity pools.

use crate::credit_line::{CreditLineError, EligibilityResult, PoolMatchingResult};
use crate::merchant::MerchantRecord;
use crate::transactions::MetadataArray;
use crate::user::KycStatus;
use crate::LiquidityPool;
use crate::PoolStatus;
use serde::{Deserialize, Serialize};

/// Configuration constants for credit evaluation
pub const MIN_REVENUE_THRESHOLD: u64 = 500000; // $5,000 monthly minimum in cents
pub const MIN_TURNOVER_DAYS: u32 = 30; // Minimum days of turnover history
pub const MIN_VERIFIED_TURNOVER_DAYS: u32 = 15; // Minimum verified turnover days
pub const TURNOVER_MULTIPLIER: u64 = 20; // 20x average daily receipts
pub const MAX_AUTO_APPROVAL_LTV: u64 = 7500; // 75% in basis points
pub const MAX_LTV_RATIO: u64 = 20000; // 200% in basis points

/// Merchant daily turnover record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTurnover {
    pub date: u64,           // Unix timestamp (date only)
    pub gross_receipts: u64, // Amount in cents
    pub refunds: u64,        // Amount in cents
    pub net_receipts: u64,   // Amount in cents
    pub verified: bool,      // Whether this data is verified
    pub source: String,      // Data source (MANUAL, API, BANK_INTEGRATION)
}

/// Merchant profile for evaluation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantProfile {
    pub merchant_id: [u8; 32],
    pub industry: Option<String>,
    pub years_in_business: Option<u32>,
    pub employee_count: Option<u32>,
    pub monthly_revenue_estimate: u64, // Amount in cents
    pub business_description: Option<String>,
    pub profile_completed: bool,
}

/// Document verification status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentVerification {
    pub document_type: String,
    pub status: String, // PENDING, VERIFIED, REJECTED
    pub verified_at: Option<u64>,
}

/// Restricted MCC codes that are not eligible for credit
const RESTRICTED_MCC_CODES: &[&str] = &[
    "6012", // Financial institutions
    "7995", // Gambling
    "5993", // Cigar stores and stands
    "5921", // Package stores-beer, wine, and liquor
    "7841", // Video tape rental stores
    "7277", // Counseling services
    "8999", // Professional services (too broad)
];

/// Credit line evaluation engine
#[derive(Debug, Clone)]
pub struct CreditEvaluator {
    pub min_revenue_threshold: u64,
    pub min_turnover_days: u32,
    pub turnover_multiplier: u64,
    pub max_auto_approval_ltv: u64,
}

impl CreditEvaluator {
    /// Create new evaluator with default settings
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_revenue_threshold: MIN_REVENUE_THRESHOLD,
            min_turnover_days: MIN_TURNOVER_DAYS,
            turnover_multiplier: TURNOVER_MULTIPLIER,
            max_auto_approval_ltv: MAX_AUTO_APPROVAL_LTV,
        }
    }

    /// Validate merchant eligibility for credit lines
    pub fn validate_eligibility(
        &self,
        merchant: &MerchantRecord,
        profile: &MerchantProfile,
        turnover_data: &[DailyTurnover],
        documents: &[DocumentVerification],
        kyc_status: KycStatus,
    ) -> EligibilityResult {
        let mut result = EligibilityResult::new(false, 0);
        let mut score = 0u8;

        // KYC approval check (required)
        let kyc_approved = kyc_status == KycStatus::Approved;
        result.add_check("kyc_approved".to_string(), kyc_approved);
        if kyc_approved {
            score += 25;
        }

        // Profile completion check
        let profile_complete = profile.profile_completed
            && profile.industry.is_some()
            && profile.years_in_business.is_some()
            && profile.monthly_revenue_estimate > 0;
        result.add_check("profile_complete".to_string(), profile_complete);
        if profile_complete {
            score += 15;
        }

        // MCC restriction check
        let mcc_allowed = !is_restricted_mcc(&merchant.mcc.to_string());
        result.add_check("mcc_allowed".to_string(), mcc_allowed);
        if mcc_allowed {
            score += 10;
        }

        // Revenue threshold check
        let revenue_sufficient = profile.monthly_revenue_estimate >= self.min_revenue_threshold;
        result.add_check("revenue_sufficient".to_string(), revenue_sufficient);
        if revenue_sufficient {
            score += 15;
        }

        // Turnover history check
        let turnover_sufficient = turnover_data.len() as u32 >= self.min_turnover_days;
        result.add_check(
            "turnover_history_sufficient".to_string(),
            turnover_sufficient,
        );
        if turnover_sufficient {
            score += 15;
        }

        // Verified turnover check
        let verified_days = turnover_data.iter().filter(|t| t.verified).count() as u32;
        let verified_turnover_sufficient = verified_days >= MIN_VERIFIED_TURNOVER_DAYS;
        result.add_check(
            "verified_turnover_sufficient".to_string(),
            verified_turnover_sufficient,
        );
        if verified_turnover_sufficient {
            score += 10;
        }

        // Document verification check
        let bank_statement_verified = documents
            .iter()
            .any(|d| d.document_type == "BANK_STATEMENT" && d.status == "VERIFIED");
        result.add_check(
            "bank_statement_verified".to_string(),
            bank_statement_verified,
        );
        if bank_statement_verified {
            score += 10;
        }

        result.score = score;
        result.eligible = score >= 70; // Require 70% minimum score

        result
    }

    /// Calculate collateral basis from turnover data
    #[must_use]
    pub fn calculate_collateral_basis(&self, turnover_data: &[DailyTurnover]) -> u64 {
        if turnover_data.is_empty() {
            return 0;
        }

        // Only use verified turnover data for collateral calculation
        let verified_turnover: Vec<_> = turnover_data.iter().filter(|t| t.verified).collect();

        if verified_turnover.is_empty() {
            return 0;
        }

        // Calculate average daily net receipts
        let total_receipts: u64 = verified_turnover.iter().map(|t| t.net_receipts).sum();
        let avg_daily_receipts = total_receipts / verified_turnover.len() as u64;

        // Apply multiplier (default 20x average daily receipts)
        avg_daily_receipts * self.turnover_multiplier
    }

    /// Calculate LTV ratio from request amount and collateral basis
    #[must_use]
    pub fn calculate_ltv_ratio(&self, requested_amount: u64, collateral_basis: u64) -> u64 {
        if collateral_basis == 0 {
            return u64::MAX; // Invalid - infinite LTV
        }

        // LTV = (requested_amount / collateral_basis) * 10000 (basis points)
        (requested_amount * 10000) / collateral_basis
    }

    /// Find matching pools for a credit request
    pub fn find_matching_pools(
        &self,
        requested_amount: u64,
        ltv_ratio: u64,
        max_rate: Option<u64>,
        available_pools: &[LiquidityPool],
    ) -> PoolMatchingResult {
        let collateral_basis = if ltv_ratio > 0 && requested_amount > 0 {
            (requested_amount * 10000) / ltv_ratio
        } else {
            0
        };

        // Filter pools that match criteria
        let matching_pools: Vec<_> = available_pools
            .iter()
            .filter(|pool| {
                pool.status == PoolStatus::Active  // was crate::loan::PoolStatus::Active
                    && pool.available_liquidity >= requested_amount
                    && ltv_ratio <= (pool.deployment_terms.max_ltv_ratio as u64 * 100) // Convert to basis points
                    && max_rate
                        .map_or(true, |max_r| (pool.deployment_terms.max_interest_rate as u64 * 100) <= max_r)
            })
            .cloned()
            .collect();

        // Select best pool (lowest rate, then highest liquidity)
        let selected_pool = matching_pools
            .iter()
            .min_by(|a, b| {
                a.deployment_terms
                    .max_interest_rate
                    .cmp(&b.deployment_terms.max_interest_rate)
                    .then(b.available_liquidity.cmp(&a.available_liquidity))
            })
            .cloned();

        let approval_recommended =
            selected_pool.is_some() && ltv_ratio <= self.max_auto_approval_ltv;

        // Calculate match score based on various factors
        let match_score = if let Some(pool) = &selected_pool {
            let mut score = 0u8;

            // Liquidity coverage (40 points max)
            if pool.available_liquidity >= requested_amount * 2 {
                score += 40;
            } else if pool.available_liquidity >= requested_amount {
                score += 20;
            }

            // LTV ratio quality (30 points max)
            if ltv_ratio <= 5000 {
                // <= 50%
                score += 30;
            } else if ltv_ratio <= 7500 {
                // <= 75%
                score += 20;
            } else if ltv_ratio <= 10000 {
                // <= 100%
                score += 10;
            }

            // Rate competitiveness (30 points max)
            let rate_bp = pool.deployment_terms.max_interest_rate; // Already in basis points
            if rate_bp <= 800 {
                // <= 8%
                score += 30;
            } else if rate_bp <= 1200 {
                // <= 12%
                score += 20;
            } else if rate_bp <= 1800 {
                // <= 18%
                score += 10;
            }

            score
        } else {
            0
        };

        PoolMatchingResult {
            collateral_basis,
            ltv_ratio,
            matching_pools_count: matching_pools.len() as u32,
            selected_pool,
            approval_recommended,
            match_score,
        }
    }

    /// Generate evaluation notes as metadata
    pub fn generate_evaluation_notes(
        &self,
        eligibility: &EligibilityResult,
        matching: &PoolMatchingResult,
    ) -> MetadataArray {
        let notes = serde_json::json!({
            "eligibility_score": eligibility.score,
            "eligibility_checks": eligibility.checks,
            "ltv_ratio_bp": matching.ltv_ratio,
            "collateral_basis": matching.collateral_basis,
            "matching_pools": matching.matching_pools_count,
            "match_score": matching.match_score,
            "auto_approval": matching.approval_recommended
        });

        let notes_str = notes.to_string();
        MetadataArray::from_bytes(notes_str.as_bytes())
    }
}

impl Default for CreditEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if MCC is restricted
#[must_use]
pub fn is_restricted_mcc(mcc: &str) -> bool {
    RESTRICTED_MCC_CODES.contains(&mcc)
}

/// Get required documents for MCC
#[must_use]
pub fn get_required_documents_for_mcc(mcc: &str) -> Vec<String> {
    let mut docs = vec!["BANK_STATEMENT".to_string()];

    match mcc {
        // High-risk industries require additional documentation
        "5812" | "5813" => {
            // Restaurants
            docs.push("BUSINESS_LICENSE".to_string());
            docs.push("FINANCIAL_STATEMENT".to_string());
        }
        "5541" | "5542" => {
            // Gas stations
            docs.push("BUSINESS_LICENSE".to_string());
        }
        "5999" => {
            // Miscellaneous retail
            docs.push("TAX_RETURN".to_string());
        }
        _ => {
            // Standard requirements for most merchants
            if mcc.starts_with('7') {
                // Services
                docs.push("BUSINESS_LICENSE".to_string());
            }
        }
    }

    docs
}

/// Validate all required documents are verified
pub fn validate_required_documents(
    mcc: &str,
    documents: &[DocumentVerification],
) -> (bool, Vec<String>) {
    let required = get_required_documents_for_mcc(mcc);
    let mut missing = Vec::new();

    for req_doc in &required {
        if !documents
            .iter()
            .any(|d| d.document_type == *req_doc && d.status == "VERIFIED")
        {
            missing.push(req_doc.clone());
        }
    }

    (missing.is_empty(), missing)
}

/// Calculate 30-day moving average from turnover data
#[must_use]
pub fn calculate_30_day_average(turnover_data: &[DailyTurnover]) -> u64 {
    if turnover_data.is_empty() {
        return 0;
    }

    // Sort by date (most recent first) and take last 30 entries
    let mut sorted_data = turnover_data.to_vec();
    sorted_data.sort_by(|a, b| b.date.cmp(&a.date));

    let recent_data: Vec<_> = sorted_data.into_iter().take(30).collect();

    if recent_data.is_empty() {
        return 0;
    }

    let total: u64 = recent_data.iter().map(|t| t.net_receipts).sum();
    total / recent_data.len() as u64
}

/// Comprehensive credit evaluation workflow
pub fn evaluate_credit_request(
    merchant: &MerchantRecord,
    profile: &MerchantProfile,
    turnover_data: &[DailyTurnover],
    documents: &[DocumentVerification],
    kyc_status: KycStatus,
    requested_amount: u64,
    max_rate: Option<u64>,
    available_pools: &[LiquidityPool],
) -> Result<(EligibilityResult, PoolMatchingResult), CreditLineError> {
    let evaluator = CreditEvaluator::new();

    // Step 1: Validate eligibility
    let eligibility =
        evaluator.validate_eligibility(merchant, profile, turnover_data, documents, kyc_status);

    if !eligibility.eligible {
        return Ok((
            eligibility,
            PoolMatchingResult {
                collateral_basis: 0,
                ltv_ratio: u64::MAX,
                matching_pools_count: 0,
                selected_pool: None,
                approval_recommended: false,
                match_score: 0,
            },
        ));
    }

    // Step 2: Calculate collateral basis and LTV
    let collateral_basis = evaluator.calculate_collateral_basis(turnover_data);
    if collateral_basis == 0 {
        return Err(CreditLineError::InsufficientCollateral);
    }

    let ltv_ratio = evaluator.calculate_ltv_ratio(requested_amount, collateral_basis);
    if ltv_ratio > MAX_LTV_RATIO {
        return Err(CreditLineError::InvalidLtvRatio);
    }

    // Step 3: Find matching pools
    let matching =
        evaluator.find_matching_pools(requested_amount, ltv_ratio, max_rate, available_pools);

    if matching.selected_pool.is_none() {
        return Err(CreditLineError::PoolMatchingFailed);
    }

    Ok((eligibility, matching))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deal::{LiquidityPool, PoolStatus}; // was loan module
    use crate::merchant::MerchantRecord;
    use crate::user::KycStatus;

    #[test]
    fn test_restricted_mcc_check() {
        assert!(is_restricted_mcc("6012"));
        assert!(is_restricted_mcc("7995"));
        assert!(!is_restricted_mcc("5999"));
    }

    #[test]
    fn test_collateral_calculation() {
        let evaluator = CreditEvaluator::new();
        let turnover = vec![
            DailyTurnover {
                date: 1640995200, // 2022-01-01
                gross_receipts: 100000,
                refunds: 5000,
                net_receipts: 95000,
                verified: true,
                source: "VERIFIED".to_string(),
            },
            DailyTurnover {
                date: 1641081600, // 2022-01-02
                gross_receipts: 120000,
                refunds: 8000,
                net_receipts: 112000,
                verified: true,
                source: "VERIFIED".to_string(),
            },
        ];

        let collateral = evaluator.calculate_collateral_basis(&turnover);
        let expected = ((95000 + 112000) / 2) * 20; // Average * multiplier
        assert_eq!(collateral, expected);
    }

    #[test]
    fn test_ltv_calculation() {
        let evaluator = CreditEvaluator::new();
        let ltv = evaluator.calculate_ltv_ratio(100000, 500000); // $1000 / $5000
        assert_eq!(ltv, 2000); // 20% in basis points
    }
}
