//! Credit Agreements Database Operations
//!
//! Provides CRUD operations for credit agreement documents.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Credit Agreement record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CreditAgreement {
    pub id: Uuid,
    pub document_id: String,
    pub version: String,
    pub execution_status: String,
    pub executed_pdf_hash: String,
    pub bundle_hash: Option<String>,
    pub effective_date: NaiveDate,
    pub maturity_date: NaiveDate,
    pub closing_date: Option<NaiveDate>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub governing_law: String,
    pub borrower_legal_name: String,
    pub borrower_external_id: Option<String>,
    pub agent_or_lender: String,
    pub collateral_agent: Option<String>,
    pub guarantors_present: bool,
    pub guarantor_names: Option<Vec<String>>,
    pub facility_type: String,
    pub committed_amount: Decimal,
    pub currency: String,
    pub draw_rules: Option<String>,
    pub repayment_type: String,
    pub amortization_schedule: Option<serde_json::Value>,
    pub reference_rate: String,
    pub margin_bps: i32,
    pub floor_bps: Option<i32>,
    pub secured_flag: bool,
    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<serde_json::Value>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,
    pub deal_state: String,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a credit agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCreditAgreementRequest {
    pub document_id: String,
    pub version: Option<String>,
    pub execution_status: Option<String>,
    pub executed_pdf_hash: String,
    pub bundle_hash: Option<String>,
    pub effective_date: NaiveDate,
    pub maturity_date: NaiveDate,
    pub closing_date: Option<NaiveDate>,
    pub governing_law: String,
    pub borrower_legal_name: String,
    pub borrower_external_id: Option<String>,
    pub agent_or_lender: String,
    pub collateral_agent: Option<String>,
    pub guarantors_present: bool,
    pub guarantor_names: Option<Vec<String>>,
    pub facility_type: String,
    pub committed_amount: Decimal,
    pub currency: Option<String>,
    pub draw_rules: Option<String>,
    pub repayment_type: String,
    pub amortization_schedule: Option<serde_json::Value>,
    pub reference_rate: String,
    pub margin_bps: i32,
    pub floor_bps: Option<i32>,
    pub secured_flag: bool,
    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<serde_json::Value>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,
    pub deal_state: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a credit agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCreditAgreementRequest {
    pub document_id: Option<String>,
    pub version: Option<String>,
    pub execution_status: Option<String>,
    pub executed_pdf_hash: Option<String>,
    pub bundle_hash: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub maturity_date: Option<NaiveDate>,
    pub closing_date: Option<NaiveDate>,
    pub governing_law: Option<String>,
    pub borrower_legal_name: Option<String>,
    pub borrower_external_id: Option<String>,
    pub agent_or_lender: Option<String>,
    pub collateral_agent: Option<String>,
    pub guarantors_present: Option<bool>,
    pub guarantor_names: Option<Vec<String>>,
    pub facility_type: Option<String>,
    pub committed_amount: Option<Decimal>,
    pub currency: Option<String>,
    pub draw_rules: Option<String>,
    pub repayment_type: Option<String>,
    pub amortization_schedule: Option<serde_json::Value>,
    pub reference_rate: Option<String>,
    pub margin_bps: Option<i32>,
    pub floor_bps: Option<i32>,
    pub secured_flag: Option<bool>,
    pub collateral_scope_summary: Option<String>,
    pub financial_covenants: Option<serde_json::Value>,
    pub use_of_proceeds: Option<String>,
    pub accordion_provision: Option<String>,
    pub prepayment_terms: Option<String>,
    pub default_provisions: Option<String>,
    pub deal_state: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new credit agreement
#[instrument(skip(pool))]
pub async fn create_credit_agreement(
    pool: &PgPool,
    req: CreateCreditAgreementRequest,
) -> Result<CreditAgreement, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        CreditAgreement,
        r#"
        INSERT INTO credit_agreements (
            id, document_id, version, execution_status, executed_pdf_hash, bundle_hash,
            effective_date, maturity_date, closing_date, created_at, updated_at, governing_law,
            borrower_legal_name, borrower_external_id, agent_or_lender, collateral_agent,
            guarantors_present, guarantor_names, facility_type, committed_amount, currency,
            draw_rules, repayment_type, amortization_schedule, reference_rate, margin_bps,
            floor_bps, secured_flag, collateral_scope_summary, financial_covenants,
            use_of_proceeds, accordion_provision, prepayment_terms, default_provisions,
            deal_state, metadata, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
            $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
            $31, $32, $33, $34, $35, $36, $37
        )
        RETURNING
            id, document_id, version, execution_status, executed_pdf_hash, bundle_hash,
            effective_date, maturity_date, closing_date,
            created_at as "created_at!", updated_at as "updated_at!",
            governing_law, borrower_legal_name, borrower_external_id, agent_or_lender,
            collateral_agent, guarantors_present, guarantor_names, facility_type,
            committed_amount, currency, draw_rules, repayment_type, amortization_schedule,
            reference_rate, margin_bps, floor_bps, secured_flag, collateral_scope_summary,
            financial_covenants, use_of_proceeds, accordion_provision, prepayment_terms,
            default_provisions, deal_state, metadata, created_by, updated_by
        "#,
        id,
        req.document_id,
        req.version.unwrap_or_else(|| "1.0".to_string()),
        req.execution_status.unwrap_or_else(|| "draft".to_string()),
        req.executed_pdf_hash,
        req.bundle_hash,
        req.effective_date,
        req.maturity_date,
        req.closing_date,
        now,
        now,
        req.governing_law,
        req.borrower_legal_name,
        req.borrower_external_id,
        req.agent_or_lender,
        req.collateral_agent,
        req.guarantors_present,
        req.guarantor_names.as_deref(),
        req.facility_type,
        req.committed_amount,
        req.currency.unwrap_or_else(|| "USD".to_string()),
        req.draw_rules,
        req.repayment_type,
        req.amortization_schedule,
        req.reference_rate,
        req.margin_bps,
        req.floor_bps,
        req.secured_flag,
        req.collateral_scope_summary,
        req.financial_covenants,
        req.use_of_proceeds,
        req.accordion_provision,
        req.prepayment_terms,
        req.default_provisions,
        req.deal_state.unwrap_or_else(|| "draft".to_string()),
        req.metadata,
        req.created_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get credit agreement by ID
#[instrument(skip(pool))]
pub async fn get_credit_agreement_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<CreditAgreement>, DbError> {
    let record = sqlx::query_as!(
        CreditAgreement,
        r#"
        SELECT
            id, document_id, version, execution_status, executed_pdf_hash, bundle_hash,
            effective_date, maturity_date, closing_date,
            created_at as "created_at!", updated_at as "updated_at!",
            governing_law, borrower_legal_name, borrower_external_id, agent_or_lender,
            collateral_agent, guarantors_present, guarantor_names, facility_type,
            committed_amount, currency, draw_rules, repayment_type, amortization_schedule,
            reference_rate, margin_bps, floor_bps, secured_flag, collateral_scope_summary,
            financial_covenants, use_of_proceeds, accordion_provision, prepayment_terms,
            default_provisions, deal_state, metadata, created_by, updated_by
        FROM credit_agreements WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get credit agreement by document ID
#[instrument(skip(pool))]
pub async fn get_credit_agreement_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<CreditAgreement>, DbError> {
    let record = sqlx::query_as!(
        CreditAgreement,
        r#"
        SELECT
            id, document_id, version, execution_status, executed_pdf_hash, bundle_hash,
            effective_date, maturity_date, closing_date,
            created_at as "created_at!", updated_at as "updated_at!",
            governing_law, borrower_legal_name, borrower_external_id, agent_or_lender,
            collateral_agent, guarantors_present, guarantor_names, facility_type,
            committed_amount, currency, draw_rules, repayment_type, amortization_schedule,
            reference_rate, margin_bps, floor_bps, secured_flag, collateral_scope_summary,
            financial_covenants, use_of_proceeds, accordion_provision, prepayment_terms,
            default_provisions, deal_state, metadata, created_by, updated_by
        FROM credit_agreements WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List credit agreements with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_credit_agreements(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    deal_state: Option<String>,
    execution_status: Option<String>,
    facility_type: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<CreditAgreement>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    if deal_state.is_some() {
        where_clauses.push("deal_state = $3");
    }
    if execution_status.is_some() {
        where_clauses.push("execution_status = $4");
    }
    if facility_type.is_some() {
        where_clauses.push("facility_type = $5");
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("effective_date") => "effective_date",
        Some("maturity_date") => "maturity_date",
        Some("borrower_legal_name") => "borrower_legal_name",
        Some("committed_amount") => "committed_amount",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM credit_agreements {}",
        where_clause
    );

    let total: i64 = sqlx::query_scalar(&count_query)
        .bind(&deal_state)
        .bind(&execution_status)
        .bind(&facility_type)
        .fetch_one(pool)
        .await?;

    // Get records
    let query = format!(
        "SELECT * FROM credit_agreements {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let records = sqlx::query_as::<_, CreditAgreement>(&query)
        .bind(limit)
        .bind(offset)
        .bind(&deal_state)
        .bind(&execution_status)
        .bind(&facility_type)
        .fetch_all(pool)
        .await?;

    Ok((records, total))
}

/// Update credit agreement (full replacement)
#[instrument(skip(pool))]
pub async fn update_credit_agreement(
    pool: &PgPool,
    id: Uuid,
    req: UpdateCreditAgreementRequest,
    updated_by: Option<Uuid>,
) -> Result<CreditAgreement, DbError> {
    // Get existing record
    let existing = get_credit_agreement_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Credit agreement {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        CreditAgreement,
        r#"
        UPDATE credit_agreements SET
            document_id = $2,
            version = $3,
            execution_status = $4,
            executed_pdf_hash = $5,
            bundle_hash = $6,
            effective_date = $7,
            maturity_date = $8,
            closing_date = $9,
            governing_law = $10,
            borrower_legal_name = $11,
            borrower_external_id = $12,
            agent_or_lender = $13,
            collateral_agent = $14,
            guarantors_present = $15,
            guarantor_names = $16,
            facility_type = $17,
            committed_amount = $18,
            currency = $19,
            draw_rules = $20,
            repayment_type = $21,
            amortization_schedule = $22,
            reference_rate = $23,
            margin_bps = $24,
            floor_bps = $25,
            secured_flag = $26,
            collateral_scope_summary = $27,
            financial_covenants = $28,
            use_of_proceeds = $29,
            accordion_provision = $30,
            prepayment_terms = $31,
            default_provisions = $32,
            deal_state = $33,
            metadata = $34,
            updated_by = $35
        WHERE id = $1
        RETURNING
            id, document_id, version, execution_status, executed_pdf_hash, bundle_hash,
            effective_date, maturity_date, closing_date,
            created_at as "created_at!", updated_at as "updated_at!",
            governing_law, borrower_legal_name, borrower_external_id, agent_or_lender,
            collateral_agent, guarantors_present, guarantor_names, facility_type,
            committed_amount, currency, draw_rules, repayment_type, amortization_schedule,
            reference_rate, margin_bps, floor_bps, secured_flag, collateral_scope_summary,
            financial_covenants, use_of_proceeds, accordion_provision, prepayment_terms,
            default_provisions, deal_state, metadata, created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.version.unwrap_or(existing.version),
        req.execution_status.unwrap_or(existing.execution_status),
        req.executed_pdf_hash.unwrap_or(existing.executed_pdf_hash),
        req.bundle_hash.or(existing.bundle_hash),
        req.effective_date.unwrap_or(existing.effective_date),
        req.maturity_date.unwrap_or(existing.maturity_date),
        req.closing_date.or(existing.closing_date),
        req.governing_law.unwrap_or(existing.governing_law),
        req.borrower_legal_name
            .unwrap_or(existing.borrower_legal_name),
        req.borrower_external_id.or(existing.borrower_external_id),
        req.agent_or_lender.unwrap_or(existing.agent_or_lender),
        req.collateral_agent.or(existing.collateral_agent),
        req.guarantors_present
            .unwrap_or(existing.guarantors_present),
        req.guarantor_names
            .as_deref()
            .or(existing.guarantor_names.as_deref()),
        req.facility_type.unwrap_or(existing.facility_type),
        req.committed_amount.unwrap_or(existing.committed_amount),
        req.currency.unwrap_or(existing.currency),
        req.draw_rules.or(existing.draw_rules),
        req.repayment_type.unwrap_or(existing.repayment_type),
        req.amortization_schedule.or(existing.amortization_schedule),
        req.reference_rate.unwrap_or(existing.reference_rate),
        req.margin_bps.unwrap_or(existing.margin_bps),
        req.floor_bps.or(existing.floor_bps),
        req.secured_flag.unwrap_or(existing.secured_flag),
        req.collateral_scope_summary
            .or(existing.collateral_scope_summary),
        req.financial_covenants.or(existing.financial_covenants),
        req.use_of_proceeds.or(existing.use_of_proceeds),
        req.accordion_provision.or(existing.accordion_provision),
        req.prepayment_terms.or(existing.prepayment_terms),
        req.default_provisions.or(existing.default_provisions),
        req.deal_state.unwrap_or(existing.deal_state),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete credit agreement
#[instrument(skip(pool))]
pub async fn delete_credit_agreement(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM credit_agreements WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple credit agreements in a batch
#[instrument(skip(pool, requests))]
pub async fn create_credit_agreements_batch(
    pool: &PgPool,
    requests: Vec<CreateCreditAgreementRequest>,
) -> Result<Vec<Result<CreditAgreement, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_credit_agreement(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
