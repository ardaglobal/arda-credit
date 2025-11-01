/// Document Extraction to Database Mappers
///
/// This module provides mapping functions to convert extracted document data
/// (from LLM extraction) into database-ready structures that can be inserted
/// into the specialized document tables.
///
/// For each document type, the mapper handles:
/// - Type conversions (String → enum, dates, decimals)
/// - Required field validation
/// - JSONB serialization for complex fields
/// - Database-specific formatting
use anyhow::{Context, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value as JsonValue;
use sqlx::types::Json;
use std::str::FromStr;
use uuid::Uuid;

// NOTE: Import extraction structs from comprehensive_schema when integrating
// The extraction structs are defined in: api/src/services/document_ingestion/comprehensive_schema.rs
//
// In your ingestion pipeline, you'll need to:
// 1. Import the extraction structs from the api crate
// 2. Pass the extracted data to these mapping functions
// 3. Use the returned DbRecord structs for database insertion
//
// Example integration:
// ```rust
// use api::services::document_ingestion::comprehensive_schema::CreditAgreementExtraction;
// use db::document_mappers::map_credit_agreement_to_db;
//
// let extraction: CreditAgreementExtraction = /* ... LLM extraction result ... */;
// let db_record = map_credit_agreement_to_db(&extraction, "CA-2025-001", None)?;
// // Insert db_record into credit_agreements table
// ```
//
// For now, we'll define placeholder types to allow compilation:

// =============================================================================
// Credit Agreement Mapper
// =============================================================================

/// Maps CreditAgreementExtraction to credit_agreements table fields
///
/// # Returns
/// A tuple of all fields ready for database insertion
pub fn map_credit_agreement_to_db(
    extraction: &CreditAgreementExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<CreditAgreementDbRecord> {
    // Parse dates
    let effective_date = parse_date_opt(&extraction.effective_date)
        .context("Failed to parse effective_date")?;
    let maturity_date =
        parse_date_opt(&extraction.maturity_date).context("Failed to parse maturity_date")?;

    // Convert enums to database string format
    let execution_status = extraction
        .execution_status
        .as_ref()
        .map(|e| format!("{:?}", e).to_lowercase());

    let facility_type = extraction
        .facility_type
        .as_ref()
        .map(|e| format!("{:?}", e).to_lowercase());

    let repayment_type = extraction
        .repayment_type
        .as_ref()
        .map(|e| format!("{:?}", e).to_lowercase());

    let reference_rate = extraction
        .reference_rate
        .as_ref()
        .map(|e| format!("{:?}", e).to_lowercase());

    // Convert amounts to Decimal
    let committed_amount =
        Decimal::from_f64(extraction.committed_amount).context("Invalid committed_amount")?;

    // Build financial covenants JSONB
    let financial_covenants = serde_json::json!({
        "covenant_1_type": extraction.financial_covenant_1_type,
        "covenant_1_level": extraction.financial_covenant_1_level,
        "test_frequency": extraction.financial_covenant_test_frequency.as_ref().map(|f| format!("{:?}", f)),
        "additional_count": extraction.additional_financial_covenants_count,
    });

    Ok(CreditAgreementDbRecord {
        id: credit_agreement_id.unwrap_or_else(Uuid::new_v4),
        document_id: document_id.to_string(),
        version: extraction.version.clone().unwrap_or_else(|| "1.0".to_string()),
        execution_status: execution_status.unwrap_or_else(|| "draft".to_string()),
        executed_pdf_hash: extraction
            .executed_pdf_hash
            .clone()
            .unwrap_or_else(|| "".to_string()),
        bundle_hash: None,
        effective_date,
        maturity_date,
        closing_date: None,
        governing_law: extraction
            .governing_law
            .clone()
            .unwrap_or_else(|| "New York".to_string()),
        borrower_legal_name: extraction.borrower_legal_name.clone(),
        borrower_external_id: None,
        agent_or_lender: extraction.agent_lender.clone(),
        collateral_agent: extraction.collateral_agent.clone(),
        guarantors_present: extraction.guarantors_exist,
        guarantor_names: extraction.guarantor_names.clone(),
        facility_type: facility_type.unwrap_or_else(|| "Other".to_string()),
        committed_amount,
        currency: extraction.currency.clone(),
        draw_rules: extraction.draw_rules.clone(),
        repayment_type: repayment_type.unwrap_or_else(|| "Bullet".to_string()),
        amortization_schedule: extraction
            .amortization_schedule
            .as_ref()
            .map(|s| serde_json::from_str(s).unwrap_or(JsonValue::String(s.clone()))),
        reference_rate: reference_rate.unwrap_or_else(|| "SOFR".to_string()),
        margin_bps: extraction.margin_bps.unwrap_or(0),
        floor_bps: extraction.floor_bps.unwrap_or(0),
        secured_flag: extraction.secured,
        collateral_scope_summary: extraction.collateral_scope_summary.clone(),
        financial_covenants: Some(Json(financial_covenants)),
        use_of_proceeds: None,
        accordion_provision: None,
        prepayment_terms: extraction.prepayment_terms.clone(),
        default_provisions: Some(extraction.events_of_default.join("; ")),
        deal_state: "draft".to_string(),
        metadata: Some(Json(serde_json::json!({
            "oid_percent": extraction.oid_percent,
            "upfront_fee_percent": extraction.upfront_fee_percent,
            "unused_commitment_fee_percent": extraction.unused_commitment_fee_percent,
            "interest_pay_frequency": extraction.interest_pay_frequency.as_ref().map(|f| format!("{:?}", f)),
            "day_count": extraction.day_count,
            "account_control_daca_required": extraction.account_control_daca_required,
            "ucc_filings_required": extraction.ucc_filings_required,
            "affirmative_covenants": extraction.affirmative_covenants_summary,
            "negative_covenants": extraction.negative_covenants_summary,
            "cure_rights_allowed": extraction.cure_rights_allowed,
            "cross_default_threshold": extraction.cross_default_threshold,
            "cure_period_days": extraction.cure_period_days,
            "borrowing_notice_lead_time_days": extraction.borrowing_notice_lead_time_days,
            "compliance_certificate_frequency": extraction.compliance_certificate_frequency.as_ref().map(|f| format!("{:?}", f)),
            "assignment_rules": extraction.assignment_rules_summary,
            "kyc_complete": extraction.kyc_complete,
            "legal_opinions_received": extraction.legal_opinions_received,
            "insurance_cois_received": extraction.insurance_cois_received,
            "corporate_approvals_received": extraction.corporate_approvals_received,
            "registry_pointer": extraction.registry_pointer_cer_id,
            "linked_term_sheet_id": extraction.linked_term_sheet_id,
        }))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// Term Sheet Mapper
// =============================================================================

/// Maps TermSheetExtraction to term_sheets table (if table exists)
/// Note: This is a placeholder - term_sheets table may need to be created
pub fn map_term_sheet_to_db(
    extraction: &TermSheetExtraction,
    document_id: &str,
) -> Result<serde_json::Value> {
    // For now, return as JSONB since term_sheets table structure isn't defined
    Ok(serde_json::json!({
        "document_id": document_id,
        "term_sheet_id": extraction.term_sheet_id,
        "version_status": extraction.version_status.as_ref().map(|v| format!("{:?}", v)),
        "issue_date": extraction.issue_date,
        "expiry_date": extraction.expiry_date,
        "binding_clauses": extraction.binding_clauses,
        "borrower_legal_name": extraction.borrower_legal_name,
        "lenders_arranger_agent": extraction.lenders_arranger_agent,
        "sponsor_parent": extraction.sponsor_parent,
        "guarantors_intended": extraction.guarantors_intended,
        "facility_types": extraction.facility_types,
        "facility_amounts": extraction.facility_amounts,
        "accordion_upsize_option": extraction.accordion_upsize_option,
        "tenor_maturity": extraction.tenor_maturity,
        "availability_draw_mechanics": extraction.availability_draw_mechanics,
        "use_of_proceeds": extraction.use_of_proceeds,
        "reference_rate": extraction.reference_rate,
        "margin_spread_bps": extraction.margin_spread_bps,
        "floor_bps": extraction.floor_bps,
        "oid_percent": extraction.oid_percent,
        "fees": extraction.fees,
        "amortization": extraction.amortization,
        "prepayment_call_protection": extraction.prepayment_call_protection,
        "secured": extraction.secured,
        "collateral_scope_intended": extraction.collateral_scope_intended,
        "account_control_daca_expected": extraction.account_control_daca_expected,
        "guarantees_intended": extraction.guarantees_intended,
        "financial_covenants_headline": extraction.financial_covenants_headline,
        "affirmative_covenants_summary": extraction.affirmative_covenants_summary,
        "negative_covenants_summary": extraction.negative_covenants_summary,
        "equity_cure_rights_intended": extraction.equity_cure_rights_intended,
        "conditions_precedent_headline": extraction.conditions_precedent_headline,
        "documentation_standard": extraction.documentation_standard.as_ref().map(|d| format!("{:?}", d)),
        "target_signing_funding_dates": extraction.target_signing_funding_dates,
        "exclusivity_no_shop": extraction.exclusivity_no_shop,
        "governing_law_venue": extraction.governing_law_venue,
        "term_sheet_pdf_hash": extraction.term_sheet_pdf_hash,
        "linked_credit_agreement_id": extraction.linked_credit_agreement_id,
        "notes": extraction.notes,
    }))
}

// =============================================================================
// Security Agreement Mapper
// =============================================================================

/// Maps SecurityAgreementExtraction to security_agreements table
pub fn map_security_agreement_to_db(
    extraction: &SecurityAgreementExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<SecurityAgreementDbRecord> {
    let effective_date = parse_date_opt(&extraction.effective_date)
        .context("Failed to parse effective_date")?;
    let termination_date = parse_date_opt(&extraction.termination_date)
        .context("Failed to parse termination_date")?;
    let perfection_date = parse_date_opt(&extraction.perfection_date)
        .context("Failed to parse perfection_date")?;

    let perfection_status = extraction
        .perfection_status
        .as_ref()
        .map(|p| format!("{:?}", p).to_lowercase())
        .unwrap_or_else(|| "pending".to_string());

    let lien_priority = extraction
        .lien_priority
        .as_ref()
        .map(|l| format!("{:?}", l).to_lowercase());

    Ok(SecurityAgreementDbRecord {
        id: Uuid::new_v4(),
        document_id: document_id.to_string(),
        credit_agreement_id,
        doc_hash: extraction
            .doc_hash
            .clone()
            .unwrap_or_else(|| "".to_string()),
        grantor: extraction.grantor.clone(),
        grantor_jurisdiction: extraction.grantor_jurisdiction.clone(),
        secured_party: extraction.secured_party.clone(),
        collateral_agent: extraction.collateral_agent.clone(),
        collateral_scope_summary: extraction.collateral_scope_summary.clone(),
        collateral_description: extraction.collateral_description.clone(),
        includes_accounts_receivable: extraction.includes_accounts_receivable,
        includes_inventory: extraction.includes_inventory,
        includes_equipment: extraction.includes_equipment,
        includes_intellectual_property: extraction.includes_intellectual_property,
        includes_real_estate: extraction.includes_real_estate,
        includes_deposit_accounts: extraction.includes_deposit_accounts,
        includes_investment_property: extraction.includes_investment_property,
        includes_general_intangibles: extraction.includes_general_intangibles,
        collateral_details: extraction.collateral_details.as_ref().map(|d| Json(d.clone())),
        perfection_methods: extraction.perfection_methods.clone(),
        perfection_status,
        perfection_date,
        lien_priority,
        intercreditor_agreement_id: extraction.intercreditor_agreement_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
        governing_law: extraction.governing_law.clone(),
        filing_jurisdictions: extraction.filing_jurisdictions.clone(),
        excluded_collateral: extraction.excluded_collateral.clone(),
        permitted_liens: extraction.permitted_liens.clone(),
        effective_date,
        termination_date,
        release_conditions: extraction.release_conditions.clone(),
        partial_release_permitted: extraction.partial_release_permitted,
        maintenance_covenants: extraction.maintenance_covenants.clone(),
        insurance_requirements: extraction.insurance_requirements.clone(),
        notes: None,
        metadata: Some(Json(serde_json::json!({}))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// Fee Letter Mapper
// =============================================================================

/// Maps FeeLetterExtraction to fee_letters table
pub fn map_fee_letter_to_db(
    extraction: &FeeLetterExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<FeeLetterDbRecord> {
    let effective_date =
        parse_date_opt(&extraction.effective_date).context("Failed to parse effective_date")?;
    let expiry_date =
        parse_date_opt(&extraction.expiry_date).context("Failed to parse expiry_date")?;

    let upfront_fee_pct =
        extraction.upfront_fee_pct.and_then(|f| Decimal::from_f64(f));
    let original_issue_discount_pct = extraction
        .original_issue_discount_pct
        .and_then(|f| Decimal::from_f64(f));
    let agency_fee_amount =
        extraction.agency_fee_amount.and_then(|f| Decimal::from_f64(f));
    let amendment_fee_amount =
        extraction.amendment_fee_amount.and_then(|f| Decimal::from_f64(f));

    Ok(FeeLetterDbRecord {
        id: Uuid::new_v4(),
        document_id: document_id.to_string(),
        credit_agreement_id,
        doc_hash_fee_letter: extraction
            .doc_hash_fee_letter
            .clone()
            .unwrap_or_else(|| "".to_string()),
        fee_components: extraction.fee_components.as_ref().map(|f| Json(f.clone())),
        confidentiality_flag: extraction.confidentiality_flag,
        upfront_fee_pct,
        original_issue_discount_pct,
        unused_commitment_fee_bps: extraction.unused_commitment_fee_bps,
        agency_fee_amount,
        amendment_fee_amount,
        fee_payment_schedule: extraction.fee_payment_schedule.clone(),
        fee_calculation_basis: extraction.fee_calculation_basis.clone(),
        effective_date,
        expiry_date,
        notes: extraction.notes.clone(),
        metadata: Some(Json(serde_json::json!({}))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// Legal Opinion Mapper
// =============================================================================

/// Maps LegalOpinionExtraction to legal_opinions table
pub fn map_legal_opinion_to_db(
    extraction: &LegalOpinionExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<LegalOpinionDbRecord> {
    let opinion_date =
        parse_date_opt(&extraction.opinion_date).context("Failed to parse opinion_date")?;
    let effective_date =
        parse_date_opt(&extraction.effective_date).context("Failed to parse effective_date")?;

    let coverage_status = extraction
        .coverage_status
        .clone()
        .unwrap_or_else(|| "complete".to_string());

    Ok(LegalOpinionDbRecord {
        id: Uuid::new_v4(),
        document_id: document_id.to_string(),
        credit_agreement_id,
        doc_hash_legal_opinion: extraction.doc_hash_legal_opinion.clone(),
        opinion_firms: extraction.opinion_firms.clone(),
        lead_opinion_firm: extraction.lead_opinion_firm.clone(),
        scopes: extraction.scopes.clone(),
        governing_laws: extraction.governing_laws.clone(),
        primary_governing_law: extraction.primary_governing_law.clone(),
        reliance_language_present: extraction.reliance_language_present,
        beneficiaries: extraction.beneficiaries.clone(),
        opinion_date,
        effective_date,
        limitations: extraction.limitations.clone(),
        assumptions: extraction.assumptions.clone(),
        full_opinion_delivered: extraction.full_opinion_delivered,
        disclosure_schedules_attached: extraction.disclosure_schedules_attached,
        bring_down_opinion_id: None,
        is_bring_down: extraction.is_bring_down,
        original_opinion_id: extraction.original_opinion_id.as_ref().and_then(|s| Uuid::parse_str(s).ok()),
        coverage_status,
        gaps_or_qualifications: extraction.gaps_or_qualifications.clone(),
        notes: None,
        metadata: Some(Json(serde_json::json!({}))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// Guaranty Mapper
// =============================================================================

/// Maps GuarantyExtraction to guaranties table
pub fn map_guaranty_to_db(
    extraction: &GuarantyExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<GuarantyDbRecord> {
    let effective_date =
        parse_date_opt(&extraction.effective_date).context("Failed to parse effective_date")?;
    let termination_date = parse_date_opt(&extraction.termination_date)
        .context("Failed to parse termination_date")?;

    let guarantee_type = extraction
        .guarantee_type
        .as_ref()
        .map(|g| format!("{:?}", g).to_lowercase())
        .unwrap_or_else(|| "full_unconditional".to_string());

    let cap_amount = extraction.cap_amount.and_then(|f| Decimal::from_f64(f));

    Ok(GuarantyDbRecord {
        id: Uuid::new_v4(),
        document_id: document_id.to_string(),
        credit_agreement_id,
        doc_hash: extraction.doc_hash.clone().unwrap_or_else(|| "".to_string()),
        guarantor_name: extraction.guarantor_name.clone(),
        guarantor_type: extraction.guarantor_type.clone(),
        guarantor_jurisdiction: extraction.guarantor_jurisdiction.clone(),
        guarantor_relationship: extraction.guarantor_relationship.clone(),
        is_joint_guaranty: extraction.is_joint_guaranty,
        other_guarantors: extraction.other_guarantors.clone(),
        beneficiary: extraction.beneficiary.clone(),
        guarantee_type,
        coverage_scope: extraction.coverage_scope.clone(),
        covers_principal: extraction.covers_principal,
        covers_interest: extraction.covers_interest,
        covers_fees: extraction.covers_fees,
        covers_expenses: extraction.covers_expenses,
        covers_amendments: extraction.covers_amendments,
        is_capped: extraction.is_capped,
        cap_amount,
        cap_currency: extraction.cap_currency.clone(),
        cap_type: extraction.cap_type.clone(),
        cap_formula: None,
        is_payment_guaranty: extraction.is_payment_guaranty,
        is_collection_guaranty: extraction.is_collection_guaranty,
        is_continuing: extraction.is_continuing,
        revolving_guaranty: extraction.revolving_guaranty,
        waivers_of_defenses: extraction.waivers_of_defenses.clone(),
        suretyship_defenses_waived: extraction.suretyship_defenses_waived,
        notice_of_default_waived: Some(false),
        notice_of_acceptance_waived: Some(true),
        subrogation_rights: extraction.subrogation_rights.clone(),
        subrogation_subordinated: extraction.subrogation_subordinated,
        subordination_provisions: None,
        secured_guaranty: extraction.secured_guaranty,
        guarantor_security_description: extraction.guarantor_security_description.clone(),
        guarantor_covenants: Some(Json(serde_json::json!({}))),
        release_conditions: extraction.release_conditions.clone(),
        automatic_release_trigger: extraction.automatic_release_trigger.clone(),
        partial_release_permitted: Some(false),
        effective_date,
        termination_date,
        governing_law: extraction.governing_law.clone(),
        enforceability_opinion_received: Some(false),
        opinion_reference: None,
        corporate_authority_documents: Some(Json(serde_json::json!({}))),
        guaranty_status: "pending".to_string(),
        claims_filed: Some(false),
        amount_claimed: None,
        amount_recovered: None,
        notes: None,
        metadata: Some(Json(serde_json::json!({}))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// SSI Mapper
// =============================================================================

/// Maps SsiExtraction to ssi_records table
pub fn map_ssi_to_db(
    extraction: &SsiExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<SsiDbRecord> {
    let effective_date =
        parse_date_opt(&extraction.effective_date).context("Failed to parse effective_date")?;
    let expiry_date =
        parse_date_opt(&extraction.expiry_date).context("Failed to parse expiry_date")?;

    let seller_custody_type = extraction
        .seller_custody_type
        .as_ref()
        .map(|c| format!("{:?}", c).to_lowercase())
        .unwrap_or_else(|| "bank".to_string());

    let buyer_custody_type = extraction
        .buyer_custody_type
        .as_ref()
        .map(|c| format!("{:?}", c).to_lowercase())
        .unwrap_or_else(|| "bank".to_string());

    Ok(SsiDbRecord {
        id: Uuid::new_v4(),
        document_id: document_id.to_string(),
        credit_agreement_id,
        deal_id: None,
        doc_hash: extraction.doc_hash.clone().unwrap_or_else(|| "".to_string()),
        ssi_type: extraction
            .ssi_type
            .clone()
            .unwrap_or_else(|| "full_bilateral".to_string()),
        // Seller SSI
        seller_ssi_id: extraction.seller_ssi_id.clone(),
        seller_beneficiary_name: extraction.seller_beneficiary_name.clone(),
        seller_bank_name: extraction.seller_bank_name.clone(),
        seller_bank_identifier: extraction.seller_bank_identifier.clone(),
        seller_reference_text: extraction.seller_reference_text.clone(),
        seller_custody_type,
        seller_custodian_name: extraction.seller_custodian_name.clone(),
        seller_custodian_account_ref: extraction.seller_custodian_account_ref.clone(),
        seller_wallet_chain: extraction.seller_wallet_chain.clone(),
        seller_wallet_chain_id: extraction.seller_wallet_chain_id,
        // Buyer SSI
        buyer_ssi_id: extraction.buyer_ssi_id.clone(),
        buyer_beneficiary_name: extraction.buyer_beneficiary_name.clone(),
        buyer_bank_name: extraction.buyer_bank_name.clone(),
        buyer_bank_identifier: extraction.buyer_bank_identifier.clone(),
        buyer_reference_text: extraction.buyer_reference_text.clone(),
        buyer_custody_type,
        buyer_custodian_name: extraction.buyer_custodian_name.clone(),
        buyer_custodian_account_ref: extraction.buyer_custodian_account_ref.clone(),
        buyer_wallet_chain: extraction.buyer_wallet_chain.clone(),
        buyer_wallet_chain_id: extraction.buyer_wallet_chain_id,
        // Dates and status
        effective_date,
        expiry_date,
        seller_ssi_verified: false,
        buyer_ssi_verified: false,
        ssi_status: "draft".to_string(),
        settlement_currency: extraction.settlement_currency.clone(),
        settlement_method: extraction.settlement_method.clone(),
        special_instructions: extraction.special_instructions.clone(),
        metadata: Some(Json(serde_json::json!({}))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// Secondary Trading Mapper
// =============================================================================

/// Maps SecondaryTradingExtraction to secondary_trades table
pub fn map_secondary_trading_to_db(
    extraction: &SecondaryTradingExtraction,
    document_id: &str,
    credit_agreement_id: Option<Uuid>,
) -> Result<SecondaryTradingDbRecord> {
    let trade_date = parse_date_opt(&extraction.trade_date).context("Failed to parse trade_date")?;
    let effective_date =
        parse_date_opt(&extraction.effective_date).context("Failed to parse effective_date")?;
    let settlement_date = parse_date_opt(&extraction.settlement_date)
        .context("Failed to parse settlement_date")?;

    let trade_type = extraction
        .trade_type
        .as_ref()
        .map(|t| format!("{:?}", t).to_lowercase())
        .unwrap_or_else(|| "par_trade".to_string());

    let price_type = extraction
        .price_type
        .as_ref()
        .map(|p| format!("{:?}", p).to_lowercase())
        .unwrap_or_else(|| "percentage_of_par".to_string());

    let notional_amount = Decimal::from_f64(extraction.notional_amount)
        .context("Invalid notional_amount")?;
    let price = Decimal::from_f64(extraction.price).context("Invalid price")?;
    let price_in_dollars =
        extraction.price_in_dollars.and_then(|f| Decimal::from_f64(f));
    let accrued_interest_amount = extraction
        .accrued_interest_amount
        .and_then(|f| Decimal::from_f64(f));
    let transfer_fee_amount =
        extraction.transfer_fee_amount.and_then(|f| Decimal::from_f64(f));

    // Calculate total consideration
    let total_consideration = price_in_dollars
        .unwrap_or(Decimal::ZERO)
        + accrued_interest_amount.unwrap_or(Decimal::ZERO);

    // Parse seller/buyer party IDs
    let seller_party_id = Uuid::parse_str(&extraction.seller_party_id)
        .context("Invalid seller_party_id UUID")?;
    let buyer_party_id =
        Uuid::parse_str(&extraction.buyer_party_id).context("Invalid buyer_party_id UUID")?;

    Ok(SecondaryTradingDbRecord {
        id: Uuid::new_v4(),
        trade_id: extraction.trade_id.clone(),
        credit_agreement_id,
        trade_confirm_hash: extraction
            .trade_confirm_hash
            .clone()
            .unwrap_or_else(|| "".to_string()),
        instrument_id: extraction.instrument_id.clone(),
        instrument_type: Some("term_loan".to_string()),
        seller_party_id,
        seller_legal_name: extraction.seller_legal_name.clone(),
        seller_role: None,
        buyer_party_id,
        buyer_legal_name: extraction.buyer_legal_name.clone(),
        buyer_role: None,
        trade_datetime: extraction.trade_datetime.clone(),
        trade_date,
        trade_type,
        trade_status: "pending".to_string(),
        effective_date,
        settlement_date,
        settlement_days: Some(3),
        actual_settlement_date: None,
        notional_amount,
        notional_currency: extraction.notional_currency.clone(),
        price_type,
        price,
        price_in_dollars,
        accrued_interest_amount,
        accrued_interest_currency: extraction
            .transfer_fee_currency
            .clone()
            .or(Some("USD".to_string())),
        total_consideration,
        total_consideration_currency: Some("USD".to_string()),
        transfer_fee_amount,
        transfer_fee_currency: extraction.transfer_fee_currency.clone(),
        eligibility_checks: extraction.eligibility_checks.as_ref().map(|e| Json(e.clone())),
        allocation_id: extraction.allocation_id.clone(),
        docs_required: extraction.docs_required.as_ref().map(|d| Json(d.clone())),
        special_terms: extraction.special_terms.clone(),
        metadata: Some(Json(serde_json::json!({}))),
        created_by: None,
        updated_by: None,
    })
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Parse optional date string (YYYY-MM-DD format)
fn parse_date_opt(date_str: &Option<String>) -> Result<Option<NaiveDate>> {
    match date_str {
        Some(s) if !s.is_empty() => {
            let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .or_else(|_| NaiveDate::parse_from_str(s, "%Y/%m/%d"))
                .or_else(|_| NaiveDate::parse_from_str(s, "%m/%d/%Y"))
                .context(format!("Failed to parse date: {}", s))?;
            Ok(Some(date))
        }
        _ => Ok(None),
    }
}

// =============================================================================
// Database Record Types
// =============================================================================

/// These structs represent the final database records ready for insertion
/// They match the exact structure of the database tables

#[derive(Debug, Clone)]
pub struct CreditAgreementDbRecord {
    pub id: Uuid,
    pub document_id: String,
    pub version: String,
    pub execution_status: String,
    pub executed_pdf_hash: String,
    pub bundle_hash: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub maturity_date: Option<NaiveDate>,
    pub closing_date: Option<NaiveDate>,
    pub governing_law: String,
    pub borrower_legal_name: String,
    pub borrower_external_id: Option<String>,
    pub agent_or_lender: String,
    pub collateral_agent: Option<String>,
    pub guarantors_present: bool,
    pub guarantor_names: Vec<String>,
    pub facility_type: String,
    pub committed_amount: Decimal,
    pub currency: String,
    pub draw_rules: Option<String>,
    pub repayment_type: String,
    pub amortization_schedule: Option<JsonValue>,
    pub reference_rate: String,
    pub margin_bps: i32,
    pub floor_bps: i32,
    pub secured_flag: bool,
    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<Json<JsonValue>>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,
    pub deal_state: String,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct SecurityAgreementDbRecord {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub grantor: String,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: String,
    pub collateral_agent: Option<String>,
    pub collateral_scope_summary: String,
    pub collateral_description: Option<String>,
    pub includes_accounts_receivable: bool,
    pub includes_inventory: bool,
    pub includes_equipment: bool,
    pub includes_intellectual_property: bool,
    pub includes_real_estate: bool,
    pub includes_deposit_accounts: bool,
    pub includes_investment_property: bool,
    pub includes_general_intangibles: bool,
    pub collateral_details: Option<Json<JsonValue>>,
    pub perfection_methods: Vec<String>,
    pub perfection_status: String,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<Uuid>,
    pub governing_law: String,
    pub filing_jurisdictions: Vec<String>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: bool,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct FeeLetterDbRecord {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_fee_letter: String,
    pub fee_components: Option<Json<JsonValue>>,
    pub confidentiality_flag: bool,
    pub upfront_fee_pct: Option<Decimal>,
    pub original_issue_discount_pct: Option<Decimal>,
    pub unused_commitment_fee_bps: Option<i32>,
    pub agency_fee_amount: Option<Decimal>,
    pub amendment_fee_amount: Option<Decimal>,
    pub fee_payment_schedule: Option<String>,
    pub fee_calculation_basis: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub notes: Option<String>,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct LegalOpinionDbRecord {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_legal_opinion: Vec<String>,
    pub opinion_firms: Vec<String>,
    pub lead_opinion_firm: Option<String>,
    pub scopes: Vec<String>,
    pub governing_laws: Vec<String>,
    pub primary_governing_law: Option<String>,
    pub reliance_language_present: bool,
    pub beneficiaries: Vec<String>,
    pub opinion_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: bool,
    pub disclosure_schedules_attached: bool,
    pub bring_down_opinion_id: Option<Uuid>,
    pub is_bring_down: bool,
    pub original_opinion_id: Option<Uuid>,
    pub coverage_status: String,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct GuarantyDbRecord {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub guarantor_name: String,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: bool,
    pub other_guarantors: Vec<String>,
    pub beneficiary: String,
    pub guarantee_type: String,
    pub coverage_scope: String,
    pub covers_principal: bool,
    pub covers_interest: bool,
    pub covers_fees: bool,
    pub covers_expenses: bool,
    pub covers_amendments: bool,
    pub is_capped: bool,
    pub cap_amount: Option<Decimal>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: bool,
    pub is_collection_guaranty: bool,
    pub is_continuing: bool,
    pub revolving_guaranty: bool,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: bool,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: bool,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: bool,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<Json<JsonValue>>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: String,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<Json<JsonValue>>,
    pub guaranty_status: String,
    pub claims_filed: Option<bool>,
    pub amount_claimed: Option<Decimal>,
    pub amount_recovered: Option<Decimal>,
    pub notes: Option<String>,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct SsiDbRecord {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub doc_hash: String,
    pub ssi_type: String,
    pub seller_ssi_id: String,
    pub seller_beneficiary_name: String,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_custody_type: String,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub buyer_ssi_id: String,
    pub buyer_beneficiary_name: String,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_custody_type: String,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub seller_ssi_verified: bool,
    pub buyer_ssi_verified: bool,
    pub ssi_status: String,
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>,
    pub special_instructions: Option<String>,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct SecondaryTradingDbRecord {
    pub id: Uuid,
    pub trade_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_confirm_hash: String,
    pub instrument_id: String,
    pub instrument_type: Option<String>,
    pub seller_party_id: Uuid,
    pub seller_legal_name: Option<String>,
    pub seller_role: Option<String>,
    pub buyer_party_id: Uuid,
    pub buyer_legal_name: Option<String>,
    pub buyer_role: Option<String>,
    pub trade_datetime: Option<String>,
    pub trade_date: Option<NaiveDate>,
    pub trade_type: String,
    pub trade_status: String,
    pub effective_date: Option<NaiveDate>,
    pub settlement_date: Option<NaiveDate>,
    pub settlement_days: Option<i32>,
    pub actual_settlement_date: Option<NaiveDate>,
    pub notional_amount: Decimal,
    pub notional_currency: Option<String>,
    pub price_type: String,
    pub price: Decimal,
    pub price_in_dollars: Option<Decimal>,
    pub accrued_interest_amount: Option<Decimal>,
    pub accrued_interest_currency: Option<String>,
    pub total_consideration: Decimal,
    pub total_consideration_currency: Option<String>,
    pub transfer_fee_amount: Option<Decimal>,
    pub transfer_fee_currency: Option<String>,
    pub eligibility_checks: Option<Json<JsonValue>>,
    pub allocation_id: Option<String>,
    pub docs_required: Option<Json<JsonValue>>,
    pub special_terms: Option<String>,
    pub metadata: Option<Json<JsonValue>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_opt() {
        assert!(parse_date_opt(&Some("2025-01-15".to_string())).is_ok());
        assert!(parse_date_opt(&Some("2025/01/15".to_string())).is_ok());
        assert!(parse_date_opt(&Some("01/15/2025".to_string())).is_ok());
        assert!(parse_date_opt(&None).unwrap().is_none());
    }
}
