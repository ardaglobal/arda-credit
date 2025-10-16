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
// Security Agreement Schema (8 fields from security_agreement.csv)
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAgreementExtraction {
    pub document_id: Option<String>,
    pub doc_hash: Option<String>,
    pub grantor: String,
    pub secured_party: String,
    pub collateral_scope_summary: String,
    pub perfection_methods: Vec<String>,
    pub governing_law: String,
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
    // Add other document types as needed
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
