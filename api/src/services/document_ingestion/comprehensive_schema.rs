/// Comprehensive extraction schema matching all MVP CSV field definitions
///
/// This module defines complete extraction schemas for all document types
/// defined in mvp_docs/*.csv files:
/// - Credit Agreement (52 fields)
/// - Term Sheet (39 fields)
/// - Security Agreement (8 fields)
/// - Guaranty (6 fields)
/// - Legal Opinion (6 fields)
/// - Fee Letter (4 fields)
/// - SSI (20 fields)
/// - Secondary Trading (18 fields)
use serde::{Deserialize, Serialize};

// =============================================================================
// Credit Agreement Schema (52 fields from Credit Agreement.csv)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditAgreementExtraction {
    // Core Identifiers
    pub agreement_id: Option<String>,
    pub version: Option<String>,
    pub execution_status: Option<ExecutionStatus>,
    pub effective_date: Option<String>,
    pub maturity_date: Option<String>,
    pub governing_law: Option<String>,

    // Parties & Roles
    pub borrower_legal_name: String,
    pub agent_lender: String,
    pub guarantors_exist: bool,
    pub guarantor_names: Vec<String>,
    pub collateral_agent: Option<String>,

    // Facility & Economics
    pub facility_type: Option<FacilityType>,
    pub committed_amount: f64,
    pub currency: String,
    pub draw_rules: Option<String>,
    pub repayment_type: Option<RepaymentType>,
    pub reference_rate: Option<ReferenceRate>,
    pub margin_bps: Option<i32>,
    pub floor_bps: Option<i32>,
    pub oid_percent: Option<f64>,
    pub upfront_fee_percent: Option<f64>,
    pub unused_commitment_fee_percent: Option<f64>,
    pub interest_pay_frequency: Option<PaymentFrequency>,
    pub day_count: Option<String>,
    pub amortization_schedule: Option<String>,
    pub prepayment_terms: Option<String>,

    // Collateral & Support
    pub secured: bool,
    pub collateral_scope_summary: Option<String>,
    pub account_control_daca_required: bool,
    pub ucc_filings_required: bool,

    // Covenants
    pub financial_covenant_1_type: Option<String>,
    pub financial_covenant_1_level: Option<String>,
    pub financial_covenant_test_frequency: Option<TestFrequency>,
    pub additional_financial_covenants_count: Option<i32>,
    pub affirmative_covenants_summary: Option<String>,
    pub negative_covenants_summary: Option<String>,
    pub cure_rights_allowed: bool,

    // Defaults & Remedies
    pub events_of_default: Vec<String>,
    pub cross_default_threshold: Option<f64>,
    pub cure_period_days: Option<i32>,

    // Ops & Admin
    pub borrowing_notice_lead_time_days: Option<i32>,
    pub compliance_certificate_frequency: Option<TestFrequency>,
    pub reporting_calendar_link: Option<String>,
    pub assignment_rules_summary: Option<String>,

    // Conditions Precedent
    pub kyc_complete: bool,
    pub legal_opinions_received: bool,
    pub insurance_cois_received: bool,
    pub corporate_approvals_received: bool,

    // Linkage & Provenance
    pub executed_pdf_hash: Option<String>,
    pub registry_pointer_cer_id: Option<String>,
    pub linked_term_sheet_id: Option<String>,
}

// =============================================================================
// Term Sheet Schema (39 fields from term_sheet.csv)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermSheetExtraction {
    // Core Identifiers
    pub term_sheet_id: Option<String>,
    pub version_status: Option<TermSheetStatus>,
    pub issue_date: Option<String>,
    pub expiry_date: Option<String>,
    pub binding_clauses: Vec<String>,

    // Parties & Roles
    pub borrower_legal_name: String,
    pub lenders_arranger_agent: Vec<String>,
    pub sponsor_parent: Option<String>,
    pub guarantors_intended: Vec<String>,

    // Facility Structure
    pub facility_types: Vec<String>,
    pub facility_amounts: Vec<f64>,
    pub accordion_upsize_option: Option<String>,
    pub tenor_maturity: Option<String>,
    pub availability_draw_mechanics: Option<String>,
    pub use_of_proceeds: Option<String>,

    // Pricing & Economics
    pub reference_rate: Option<String>,
    pub margin_spread_bps: Option<i32>,
    pub floor_bps: Option<i32>,
    pub oid_percent: Option<f64>,
    pub fees: Option<serde_json::Value>,
    pub amortization: Option<String>,
    pub prepayment_call_protection: Option<String>,

    // Security & Support
    pub secured: bool,
    pub collateral_scope_intended: Option<String>,
    pub account_control_daca_expected: bool,
    pub guarantees_intended: Option<String>,

    // Covenants
    pub financial_covenants_headline: Option<serde_json::Value>,
    pub affirmative_covenants_summary: Option<String>,
    pub negative_covenants_summary: Option<String>,
    pub equity_cure_rights_intended: Option<String>,

    // Conditions & Process
    pub conditions_precedent_headline: Vec<String>,
    pub documentation_standard: Option<DocumentationStandard>,
    pub target_signing_funding_dates: Option<String>,
    pub exclusivity_no_shop: Option<String>,
    pub governing_law_venue: Option<String>,

    // Digital & Linkage
    pub term_sheet_pdf_hash: Option<String>,
    pub linked_credit_agreement_id: Option<String>,
    pub notes: Option<String>,
}

// =============================================================================
// Security Agreement Schema (Expanded to match security_agreements table)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAgreementExtraction {
    // Core Identifiers
    pub document_id: Option<String>,
    pub doc_hash: Option<String>,

    // Parties
    pub grantor: String,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: String,
    pub collateral_agent: Option<String>,

    // Collateral Scope
    pub collateral_scope_summary: String,
    pub collateral_description: Option<String>,

    // Collateral Categories (boolean flags)
    pub includes_accounts_receivable: bool,
    pub includes_inventory: bool,
    pub includes_equipment: bool,
    pub includes_intellectual_property: bool,
    pub includes_real_estate: bool,
    pub includes_deposit_accounts: bool,
    pub includes_investment_property: bool,
    pub includes_general_intangibles: bool,

    // Collateral Details (flexible JSONB)
    pub collateral_details: Option<serde_json::Value>,

    // Perfection
    pub perfection_methods: Vec<String>, // ['UCC-1', 'DACA', 'Possession', 'Control']
    pub perfection_status: Option<PerfectionStatus>,
    pub perfection_date: Option<String>,

    // Priority
    pub lien_priority: Option<LienPriority>,
    pub intercreditor_agreement_id: Option<String>,

    // Jurisdictions & Governing Law
    pub governing_law: String,
    pub filing_jurisdictions: Vec<String>,

    // Exclusions & Carve-Outs
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,

    // Effective Dates
    pub effective_date: Option<String>,
    pub termination_date: Option<String>,

    // Release Provisions
    pub release_conditions: Option<String>,
    pub partial_release_permitted: bool,

    // Maintenance Obligations
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
}

// =============================================================================
// Supporting Enums
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExecutionStatus {
    Draft,
    Executed,
    Amended,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FacilityType {
    TermLoan,
    Revolver,
    DelayedDraw,
    BridgeLoan,
    LetterOfCredit,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RepaymentType {
    Bullet,
    Amortizing,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReferenceRate {
    Sofr,
    BaseRate,
    Euribor,
    Libor,
    Prime,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentFrequency {
    Daily,
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Semiannually,
    Annually,
    OnMaturity,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TestFrequency {
    Monthly,
    Quarterly,
    Semiannual,
    Annual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TermSheetStatus {
    Draft,
    MarkedUp,
    Agreed,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocumentationStandard {
    Lsta,
    Lma,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerfectionStatus {
    Perfected,
    Pending,
    Partial,
    NotRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LienPriority {
    FirstLien,
    SecondLien,
    Junior,
    PariPassu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuaranteeType {
    FullUnconditional,
    Limited,
    Capped,
    Payment,
    Collection,
    Performance,
    Completion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustodyType {
    Bank,
    SelfCustody,
    Custodian,
    Exchange,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeType {
    ParTrade,
    DiscountTrade,
    PremiumTrade,
    DistressedTrade,
    Participation,
    Assignment,
    Novation,
    Syndication,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceType {
    PercentageOfPar,
    DollarPrice,
    YieldBased,
    DiscountToPar,
    PremiumToPar,
}

// =============================================================================
// Fee Letter Schema
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeLetterExtraction {
    // Core Identifiers
    pub document_id: Option<String>,
    pub doc_hash_fee_letter: Option<String>,

    // Fee Components (JSONB flexible structure)
    pub fee_components: Option<serde_json::Value>,

    // Confidentiality
    pub confidentiality_flag: bool,

    // Individual Fee Breakdown (optional detailed fields)
    pub upfront_fee_pct: Option<f64>,
    pub original_issue_discount_pct: Option<f64>,
    pub unused_commitment_fee_bps: Option<i32>,
    pub agency_fee_amount: Option<f64>,
    pub amendment_fee_amount: Option<f64>,

    // Additional Details
    pub fee_payment_schedule: Option<String>,
    pub fee_calculation_basis: Option<String>,
    pub effective_date: Option<String>,
    pub expiry_date: Option<String>,
    pub notes: Option<String>,
}

// =============================================================================
// Legal Opinion Schema
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalOpinionExtraction {
    // Core Identifiers
    pub document_id: Option<String>,
    pub doc_hash_legal_opinion: Vec<String>, // Array of hashes for multiple opinions

    // Opinion Firms
    pub opinion_firms: Vec<String>, // Array of issuing law firms
    pub lead_opinion_firm: Option<String>,

    // Opinion Scopes
    pub scopes: Vec<String>, // ['Enforceability', 'Authorization', 'Perfection', etc.]

    // Governing Laws
    pub governing_laws: Vec<String>, // ['NY', 'DE', 'DE UCC']
    pub primary_governing_law: Option<String>,

    // Reliance
    pub reliance_language_present: bool,
    pub beneficiaries: Vec<String>,

    // Opinion Details
    pub opinion_date: Option<String>,
    pub effective_date: Option<String>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,

    // Deliverables
    pub full_opinion_delivered: bool,
    pub disclosure_schedules_attached: bool,

    // Bring-Down
    pub is_bring_down: bool,
    pub original_opinion_id: Option<String>,

    // Coverage
    pub coverage_status: Option<String>, // 'complete', 'partial', 'pending', 'waived'
    pub gaps_or_qualifications: Option<String>,
}

// =============================================================================
// Guaranty Schema
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuarantyExtraction {
    // Core Identifiers
    pub document_id: Option<String>,
    pub doc_hash: Option<String>,

    // Guarantor Information
    pub guarantor_name: String,
    pub guarantor_type: Option<String>, // 'corporate', 'individual', 'parent', 'subsidiary'
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,

    // Multiple Guarantors
    pub is_joint_guaranty: bool,
    pub other_guarantors: Vec<String>,

    // Beneficiary
    pub beneficiary: String,

    // Guarantee Type & Scope
    pub guarantee_type: Option<GuaranteeType>,
    pub coverage_scope: String,
    pub covers_principal: bool,
    pub covers_interest: bool,
    pub covers_fees: bool,
    pub covers_expenses: bool,
    pub covers_amendments: bool,

    // Cap/Limitation
    pub is_capped: bool,
    pub cap_amount: Option<f64>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>, // 'absolute', 'per_guarantor', 'aggregate'

    // Payment/Collection Type
    pub is_payment_guaranty: bool,
    pub is_collection_guaranty: bool,

    // Continuing Guaranty
    pub is_continuing: bool,
    pub revolving_guaranty: bool,

    // Defenses & Waivers
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: bool,

    // Subrogation
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: bool,

    // Security
    pub secured_guaranty: bool,
    pub guarantor_security_description: Option<String>,

    // Release
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,

    // Effectiveness
    pub effective_date: Option<String>,
    pub termination_date: Option<String>,
    pub governing_law: String,
}

// =============================================================================
// SSI (Standard Settlement Instructions) Schema
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SsiExtraction {
    // Core Identifiers
    pub document_id: Option<String>,
    pub doc_hash: Option<String>,

    // SSI Type
    pub ssi_type: Option<String>, // 'full_bilateral', 'seller_only', 'buyer_only'

    // Seller SSI Details
    pub seller_ssi_id: String,
    pub seller_beneficiary_name: String,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>, // SWIFT/BIC/ABA
    pub seller_reference_text: Option<String>,

    // Seller Digital Custody
    pub seller_custody_type: Option<CustodyType>,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,

    // Buyer SSI Details
    pub buyer_ssi_id: String,
    pub buyer_beneficiary_name: String,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_reference_text: Option<String>,

    // Buyer Digital Custody
    pub buyer_custody_type: Option<CustodyType>,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,

    // Effective Dates
    pub effective_date: Option<String>,
    pub expiry_date: Option<String>,

    // Settlement Preferences
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>, // 'wire', 'ach', 'blockchain', etc.
    pub special_instructions: Option<String>,
}

// =============================================================================
// Secondary Trading Schema
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondaryTradingExtraction {
    // Core Identifiers
    pub trade_id: String,
    pub trade_confirm_hash: Option<String>,
    pub instrument_id: String,

    // Parties
    pub seller_party_id: String,
    pub buyer_party_id: String,
    pub seller_legal_name: Option<String>,
    pub buyer_legal_name: Option<String>,

    // Trade Details
    pub trade_datetime: Option<String>,
    pub trade_date: Option<String>,
    pub trade_type: Option<TradeType>,

    // Financial Terms
    pub notional_amount: f64,
    pub notional_currency: Option<String>,
    pub price_type: Option<PriceType>,
    pub price: f64,
    pub price_in_dollars: Option<f64>,

    // Accrued Interest
    pub accrued_interest_amount: Option<f64>,

    // Settlement
    pub effective_date: Option<String>,
    pub settlement_date: Option<String>,

    // Compliance
    pub eligibility_checks: Option<serde_json::Value>, // JSONB for flexible checks

    // Allocation & Documentation
    pub allocation_id: Option<String>,
    pub docs_required: Option<serde_json::Value>, // JSONB for document tracking

    // Transfer Fees
    pub transfer_fee_amount: Option<f64>,
    pub transfer_fee_currency: Option<String>,

    // Special Terms
    pub special_terms: Option<String>,
}

// =============================================================================
// Unified Extraction Result
// =============================================================================

/// Complete document extraction result that can contain any document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComprehensiveExtractionResult {
    /// Detected document type
    pub document_type: DetectedDocumentType,

    /// Confidence score (0.0 to 1.0)
    pub confidence_score: f64,

    /// Extraction notes and warnings
    pub extraction_notes: Vec<String>,

    /// Specific document data (based on detected type)
    pub data: DocumentData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectedDocumentType {
    CreditAgreement,
    TermSheet,
    SecurityAgreement,
    Guaranty,
    LegalOpinion,
    FeeLetter,
    Ssi,
    SecondaryTrading,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "fields")]
pub enum DocumentData {
    CreditAgreement(CreditAgreementExtraction),
    TermSheet(TermSheetExtraction),
    SecurityAgreement(SecurityAgreementExtraction),
    FeeLetter(FeeLetterExtraction),
    LegalOpinion(LegalOpinionExtraction),
    Guaranty(GuarantyExtraction),
    Ssi(SsiExtraction),
    SecondaryTrading(SecondaryTradingExtraction),
    Unknown,
}

// =============================================================================
// JSON Schema Generator
// =============================================================================

/// Generate JSON schema for Credit Agreement extraction
pub fn get_credit_agreement_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            // Core Identifiers
            "agreement_id": {"type": ["string", "null"]},
            "version": {"type": ["string", "null"]},
            "execution_status": {"type": ["string", "null"], "enum": ["DRAFT", "EXECUTED", "AMENDED", "SUPERSEDED", null]},
            "effective_date": {"type": ["string", "null"]},
            "maturity_date": {"type": ["string", "null"]},
            "governing_law": {"type": ["string", "null"]},

            // Parties & Roles
            "borrower_legal_name": {"type": "string"},
            "agent_lender": {"type": "string"},
            "guarantors_exist": {"type": "boolean"},
            "guarantor_names": {"type": "array", "items": {"type": "string"}},
            "collateral_agent": {"type": ["string", "null"]},

            // Facility & Economics
            "facility_type": {"type": ["string", "null"], "enum": ["TERM_LOAN", "REVOLVER", "DELAYED_DRAW", "BRIDGE_LOAN", "LETTER_OF_CREDIT", "OTHER", null]},
            "committed_amount": {"type": "number"},
            "currency": {"type": "string"},
            "draw_rules": {"type": ["string", "null"]},
            "repayment_type": {"type": ["string", "null"], "enum": ["BULLET", "AMORTIZING", "CUSTOM", null]},
            "reference_rate": {"type": ["string", "null"], "enum": ["SOFR", "BASE_RATE", "EURIBOR", "LIBOR", "PRIME", "OTHER", null]},
            "margin_bps": {"type": ["integer", "null"]},
            "floor_bps": {"type": ["integer", "null"]},
            "oid_percent": {"type": ["number", "null"]},
            "upfront_fee_percent": {"type": ["number", "null"]},
            "unused_commitment_fee_percent": {"type": ["number", "null"]},
            "interest_pay_frequency": {"type": ["string", "null"]},
            "day_count": {"type": ["string", "null"]},
            "amortization_schedule": {"type": ["string", "null"]},
            "prepayment_terms": {"type": ["string", "null"]},

            // Collateral & Support
            "secured": {"type": "boolean"},
            "collateral_scope_summary": {"type": ["string", "null"]},
            "account_control_daca_required": {"type": "boolean"},
            "ucc_filings_required": {"type": "boolean"},

            // Covenants
            "financial_covenant_1_type": {"type": ["string", "null"]},
            "financial_covenant_1_level": {"type": ["string", "null"]},
            "financial_covenant_test_frequency": {"type": ["string", "null"]},
            "additional_financial_covenants_count": {"type": ["integer", "null"]},
            "affirmative_covenants_summary": {"type": ["string", "null"]},
            "negative_covenants_summary": {"type": ["string", "null"]},
            "cure_rights_allowed": {"type": "boolean"},

            // Defaults & Remedies
            "events_of_default": {"type": "array", "items": {"type": "string"}},
            "cross_default_threshold": {"type": ["number", "null"]},
            "cure_period_days": {"type": ["integer", "null"]},

            // Ops & Admin
            "borrowing_notice_lead_time_days": {"type": ["integer", "null"]},
            "compliance_certificate_frequency": {"type": ["string", "null"]},
            "reporting_calendar_link": {"type": ["string", "null"]},
            "assignment_rules_summary": {"type": ["string", "null"]},

            // Conditions Precedent
            "kyc_complete": {"type": "boolean"},
            "legal_opinions_received": {"type": "boolean"},
            "insurance_cois_received": {"type": "boolean"},
            "corporate_approvals_received": {"type": "boolean"},

            // Linkage & Provenance
            "executed_pdf_hash": {"type": ["string", "null"]},
            "registry_pointer_cer_id": {"type": ["string", "null"]},
            "linked_term_sheet_id": {"type": ["string", "null"]}
        },
        "required": ["borrower_legal_name", "agent_lender", "guarantors_exist", "guarantor_names", "committed_amount", "currency", "secured", "account_control_daca_required", "ucc_filings_required", "cure_rights_allowed", "events_of_default", "kyc_complete", "legal_opinions_received", "insurance_cois_received", "corporate_approvals_received"]
    })
}

/// Generate JSON schema for Term Sheet extraction
pub fn get_term_sheet_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "term_sheet_id": {"type": ["string", "null"]},
            "version_status": {"type": ["string", "null"]},
            "issue_date": {"type": ["string", "null"]},
            "expiry_date": {"type": ["string", "null"]},
            "binding_clauses": {"type": "array", "items": {"type": "string"}},
            "borrower_legal_name": {"type": "string"},
            "lenders_arranger_agent": {"type": "array", "items": {"type": "string"}},
            "sponsor_parent": {"type": ["string", "null"]},
            "guarantors_intended": {"type": "array", "items": {"type": "string"}},
            "facility_types": {"type": "array", "items": {"type": "string"}},
            "facility_amounts": {"type": "array", "items": {"type": "number"}},
            "accordion_upsize_option": {"type": ["string", "null"]},
            "tenor_maturity": {"type": ["string", "null"]},
            "availability_draw_mechanics": {"type": ["string", "null"]},
            "use_of_proceeds": {"type": ["string", "null"]},
            "reference_rate": {"type": ["string", "null"]},
            "margin_spread_bps": {"type": ["integer", "null"]},
            "floor_bps": {"type": ["integer", "null"]},
            "oid_percent": {"type": ["number", "null"]},
            "fees": {},
            "amortization": {"type": ["string", "null"]},
            "prepayment_call_protection": {"type": ["string", "null"]},
            "secured": {"type": "boolean"},
            "collateral_scope_intended": {"type": ["string", "null"]},
            "account_control_daca_expected": {"type": "boolean"},
            "guarantees_intended": {"type": ["string", "null"]},
            "financial_covenants_headline": {},
            "affirmative_covenants_summary": {"type": ["string", "null"]},
            "negative_covenants_summary": {"type": ["string", "null"]},
            "equity_cure_rights_intended": {"type": ["string", "null"]},
            "conditions_precedent_headline": {"type": "array", "items": {"type": "string"}},
            "documentation_standard": {"type": ["string", "null"]},
            "target_signing_funding_dates": {"type": ["string", "null"]},
            "exclusivity_no_shop": {"type": ["string", "null"]},
            "governing_law_venue": {"type": ["string", "null"]},
            "term_sheet_pdf_hash": {"type": ["string", "null"]},
            "linked_credit_agreement_id": {"type": ["string", "null"]},
            "notes": {"type": ["string", "null"]}
        },
        "required": ["borrower_legal_name", "lenders_arranger_agent", "binding_clauses", "guarantors_intended", "facility_types", "facility_amounts", "secured", "account_control_daca_expected", "conditions_precedent_headline"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_serialization() {
        let schema = get_credit_agreement_schema();
        assert!(schema["properties"]["borrower_legal_name"].is_object());
        assert_eq!(schema["properties"]["committed_amount"]["type"], "number");
    }
}
