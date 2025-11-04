//! Guaranties Database Operations
//!
//! Provides CRUD operations for guaranty documents.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Guaranty record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Guaranty {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub guarantor_name: String,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: Option<bool>,
    pub other_guarantors: Option<Vec<String>>,
    pub beneficiary: String,
    pub guarantee_type: String,
    pub coverage_scope: String,
    pub covers_principal: Option<bool>,
    pub covers_interest: Option<bool>,
    pub covers_fees: Option<bool>,
    pub covers_expenses: Option<bool>,
    pub covers_amendments: Option<bool>,
    pub is_capped: Option<bool>,
    pub cap_amount: Option<Decimal>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: Option<bool>,
    pub is_collection_guaranty: Option<bool>,
    pub is_continuing: Option<bool>,
    pub revolving_guaranty: Option<bool>,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: Option<bool>,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: Option<bool>,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: Option<bool>,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<serde_json::Value>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: String,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<serde_json::Value>,
    pub guaranty_status: Option<String>,
    pub claims_filed: Option<bool>,
    pub amount_claimed: Option<Decimal>,
    pub amount_recovered: Option<Decimal>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a guaranty
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateGuarantyRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub guarantor_name: String,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: Option<bool>,
    pub other_guarantors: Option<Vec<String>>,
    pub beneficiary: String,
    pub guarantee_type: String,
    pub coverage_scope: String,
    pub covers_principal: Option<bool>,
    pub covers_interest: Option<bool>,
    pub covers_fees: Option<bool>,
    pub covers_expenses: Option<bool>,
    pub covers_amendments: Option<bool>,
    pub is_capped: Option<bool>,
    pub cap_amount: Option<Decimal>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: Option<bool>,
    pub is_collection_guaranty: Option<bool>,
    pub is_continuing: Option<bool>,
    pub revolving_guaranty: Option<bool>,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: Option<bool>,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: Option<bool>,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: Option<bool>,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<serde_json::Value>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: String,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<serde_json::Value>,
    pub guaranty_status: Option<String>,
    pub claims_filed: Option<bool>,
    pub amount_claimed: Option<Decimal>,
    pub amount_recovered: Option<Decimal>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a guaranty
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateGuarantyRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: Option<String>,
    pub guarantor_name: Option<String>,
    pub guarantor_type: Option<String>,
    pub guarantor_jurisdiction: Option<String>,
    pub guarantor_relationship: Option<String>,
    pub is_joint_guaranty: Option<bool>,
    pub other_guarantors: Option<Vec<String>>,
    pub beneficiary: Option<String>,
    pub guarantee_type: Option<String>,
    pub coverage_scope: Option<String>,
    pub covers_principal: Option<bool>,
    pub covers_interest: Option<bool>,
    pub covers_fees: Option<bool>,
    pub covers_expenses: Option<bool>,
    pub covers_amendments: Option<bool>,
    pub is_capped: Option<bool>,
    pub cap_amount: Option<Decimal>,
    pub cap_currency: Option<String>,
    pub cap_type: Option<String>,
    pub cap_formula: Option<String>,
    pub is_payment_guaranty: Option<bool>,
    pub is_collection_guaranty: Option<bool>,
    pub is_continuing: Option<bool>,
    pub revolving_guaranty: Option<bool>,
    pub waivers_of_defenses: Option<String>,
    pub suretyship_defenses_waived: Option<bool>,
    pub notice_of_default_waived: Option<bool>,
    pub notice_of_acceptance_waived: Option<bool>,
    pub subrogation_rights: Option<String>,
    pub subrogation_subordinated: Option<bool>,
    pub subordination_provisions: Option<String>,
    pub secured_guaranty: Option<bool>,
    pub guarantor_security_description: Option<String>,
    pub guarantor_covenants: Option<serde_json::Value>,
    pub release_conditions: Option<String>,
    pub automatic_release_trigger: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub governing_law: Option<String>,
    pub enforceability_opinion_received: Option<bool>,
    pub opinion_reference: Option<String>,
    pub corporate_authority_documents: Option<serde_json::Value>,
    pub guaranty_status: Option<String>,
    pub claims_filed: Option<bool>,
    pub amount_claimed: Option<Decimal>,
    pub amount_recovered: Option<Decimal>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new guaranty
#[instrument(skip(pool))]
pub async fn create_guaranty(
    pool: &PgPool,
    req: CreateGuarantyRequest,
) -> Result<Guaranty, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        Guaranty,
        r#"
        INSERT INTO guaranties (
            id, document_id, credit_agreement_id, doc_hash, guarantor_name, guarantor_type,
            guarantor_jurisdiction, guarantor_relationship, is_joint_guaranty, other_guarantors,
            beneficiary, guarantee_type, coverage_scope, covers_principal, covers_interest,
            covers_fees, covers_expenses, covers_amendments, is_capped, cap_amount, cap_currency,
            cap_type, cap_formula, is_payment_guaranty, is_collection_guaranty, is_continuing,
            revolving_guaranty, waivers_of_defenses, suretyship_defenses_waived,
            notice_of_default_waived, notice_of_acceptance_waived, subrogation_rights,
            subrogation_subordinated, subordination_provisions, secured_guaranty,
            guarantor_security_description, guarantor_covenants, release_conditions,
            automatic_release_trigger, partial_release_permitted, effective_date,
            termination_date, governing_law, enforceability_opinion_received, opinion_reference,
            corporate_authority_documents, guaranty_status, claims_filed, amount_claimed,
            amount_recovered, notes, metadata, created_at, updated_at, created_by, updated_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
            $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34,
            $35, $36, $37, $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50,
            $51, $52, $53, $54, $55, $56
        )
        RETURNING
            id, document_id, credit_agreement_id, doc_hash, guarantor_name, guarantor_type,
            guarantor_jurisdiction, guarantor_relationship, is_joint_guaranty, other_guarantors,
            beneficiary, guarantee_type, coverage_scope, covers_principal, covers_interest,
            covers_fees, covers_expenses, covers_amendments, is_capped, cap_amount, cap_currency,
            cap_type, cap_formula, is_payment_guaranty, is_collection_guaranty, is_continuing,
            revolving_guaranty, waivers_of_defenses, suretyship_defenses_waived,
            notice_of_default_waived, notice_of_acceptance_waived, subrogation_rights,
            subrogation_subordinated, subordination_provisions, secured_guaranty,
            guarantor_security_description, guarantor_covenants, release_conditions,
            automatic_release_trigger, partial_release_permitted, effective_date,
            termination_date, governing_law, enforceability_opinion_received, opinion_reference,
            corporate_authority_documents, guaranty_status, claims_filed, amount_claimed,
            amount_recovered, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.doc_hash,
        req.guarantor_name,
        req.guarantor_type,
        req.guarantor_jurisdiction,
        req.guarantor_relationship,
        req.is_joint_guaranty,
        req.other_guarantors.as_deref(),
        req.beneficiary,
        req.guarantee_type,
        req.coverage_scope,
        req.covers_principal,
        req.covers_interest,
        req.covers_fees,
        req.covers_expenses,
        req.covers_amendments,
        req.is_capped,
        req.cap_amount,
        req.cap_currency,
        req.cap_type,
        req.cap_formula,
        req.is_payment_guaranty,
        req.is_collection_guaranty,
        req.is_continuing,
        req.revolving_guaranty,
        req.waivers_of_defenses,
        req.suretyship_defenses_waived,
        req.notice_of_default_waived,
        req.notice_of_acceptance_waived,
        req.subrogation_rights,
        req.subrogation_subordinated,
        req.subordination_provisions,
        req.secured_guaranty,
        req.guarantor_security_description,
        req.guarantor_covenants,
        req.release_conditions,
        req.automatic_release_trigger,
        req.partial_release_permitted,
        req.effective_date,
        req.termination_date,
        req.governing_law,
        req.enforceability_opinion_received,
        req.opinion_reference,
        req.corporate_authority_documents,
        req.guaranty_status,
        req.claims_filed,
        req.amount_claimed,
        req.amount_recovered,
        req.notes,
        req.metadata,
        now,
        now,
        req.created_by,
        None::<uuid::Uuid>, // updated_by is None on create
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get guaranty by ID
#[instrument(skip(pool))]
pub async fn get_guaranty_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Guaranty>, DbError> {
    let record = sqlx::query_as!(
        Guaranty,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash, guarantor_name, guarantor_type,
            guarantor_jurisdiction, guarantor_relationship, is_joint_guaranty, other_guarantors,
            beneficiary, guarantee_type, coverage_scope, covers_principal, covers_interest,
            covers_fees, covers_expenses, covers_amendments, is_capped, cap_amount, cap_currency,
            cap_type, cap_formula, is_payment_guaranty, is_collection_guaranty, is_continuing,
            revolving_guaranty, waivers_of_defenses, suretyship_defenses_waived,
            notice_of_default_waived, notice_of_acceptance_waived, subrogation_rights,
            subrogation_subordinated, subordination_provisions, secured_guaranty,
            guarantor_security_description, guarantor_covenants, release_conditions,
            automatic_release_trigger, partial_release_permitted, effective_date,
            termination_date, governing_law, enforceability_opinion_received, opinion_reference,
            corporate_authority_documents, guaranty_status, claims_filed, amount_claimed,
            amount_recovered, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM guaranties WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get guaranty by document ID
#[instrument(skip(pool))]
pub async fn get_guaranty_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<Guaranty>, DbError> {
    let record = sqlx::query_as!(
        Guaranty,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash, guarantor_name, guarantor_type,
            guarantor_jurisdiction, guarantor_relationship, is_joint_guaranty, other_guarantors,
            beneficiary, guarantee_type, coverage_scope, covers_principal, covers_interest,
            covers_fees, covers_expenses, covers_amendments, is_capped, cap_amount, cap_currency,
            cap_type, cap_formula, is_payment_guaranty, is_collection_guaranty, is_continuing,
            revolving_guaranty, waivers_of_defenses, suretyship_defenses_waived,
            notice_of_default_waived, notice_of_acceptance_waived, subrogation_rights,
            subrogation_subordinated, subordination_provisions, secured_guaranty,
            guarantor_security_description, guarantor_covenants, release_conditions,
            automatic_release_trigger, partial_release_permitted, effective_date,
            termination_date, governing_law, enforceability_opinion_received, opinion_reference,
            corporate_authority_documents, guaranty_status, claims_filed, amount_claimed,
            amount_recovered, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM guaranties WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List guaranties with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_guaranties(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    guarantor_type: Option<String>,
    guarantee_type: Option<String>,
    guaranty_status: Option<String>,
    is_capped: Option<bool>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<Guaranty>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_count = 3; // Start after limit, offset

    if credit_agreement_id.is_some() {
        where_clauses.push(format!("credit_agreement_id = ${}", param_count));
        param_count += 1;
    }
    if guarantor_type.is_some() {
        where_clauses.push(format!("guarantor_type = ${}", param_count));
        param_count += 1;
    }
    if guarantee_type.is_some() {
        where_clauses.push(format!("guarantee_type = ${}", param_count));
        param_count += 1;
    }
    if guaranty_status.is_some() {
        where_clauses.push(format!("guaranty_status = ${}", param_count));
        param_count += 1;
    }
    if is_capped.is_some() {
        where_clauses.push(format!("is_capped = ${}", param_count));
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("guarantor_name") => "guarantor_name",
        Some("effective_date") => "effective_date",
        Some("guaranty_status") => "guaranty_status",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!("SELECT COUNT(*) as count FROM guaranties {}", where_clause);

    let mut count_q = sqlx::query_scalar(&count_query);
    if credit_agreement_id.is_some() {
        count_q = count_q.bind(&credit_agreement_id);
    }
    if let Some(gt) = &guarantor_type {
        count_q = count_q.bind(gt);
    }
    if let Some(gt) = &guarantee_type {
        count_q = count_q.bind(gt);
    }
    if let Some(gs) = &guaranty_status {
        count_q = count_q.bind(gs);
    }
    if let Some(ic) = &is_capped {
        count_q = count_q.bind(ic);
    }

    let total: i64 = count_q.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM guaranties {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut records_q = sqlx::query_as::<_, Guaranty>(&query)
        .bind(limit)
        .bind(offset);

    if credit_agreement_id.is_some() {
        records_q = records_q.bind(&credit_agreement_id);
    }
    if let Some(gt) = &guarantor_type {
        records_q = records_q.bind(gt);
    }
    if let Some(gt) = &guarantee_type {
        records_q = records_q.bind(gt);
    }
    if let Some(gs) = &guaranty_status {
        records_q = records_q.bind(gs);
    }
    if let Some(ic) = &is_capped {
        records_q = records_q.bind(ic);
    }

    let records = records_q.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update guaranty
#[instrument(skip(pool))]
pub async fn update_guaranty(
    pool: &PgPool,
    id: Uuid,
    req: UpdateGuarantyRequest,
    updated_by: Option<Uuid>,
) -> Result<Guaranty, DbError> {
    // Get existing record
    let existing = get_guaranty_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Guaranty {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let other_guarantors = req.other_guarantors.or(existing.other_guarantors);

    let record = sqlx::query_as!(
        Guaranty,
        r#"
        UPDATE guaranties SET
            document_id = $2,
            credit_agreement_id = $3,
            doc_hash = $4,
            guarantor_name = $5,
            guarantor_type = $6,
            guarantor_jurisdiction = $7,
            guarantor_relationship = $8,
            is_joint_guaranty = $9,
            other_guarantors = $10,
            beneficiary = $11,
            guarantee_type = $12,
            coverage_scope = $13,
            covers_principal = $14,
            covers_interest = $15,
            covers_fees = $16,
            covers_expenses = $17,
            covers_amendments = $18,
            is_capped = $19,
            cap_amount = $20,
            cap_currency = $21,
            cap_type = $22,
            cap_formula = $23,
            is_payment_guaranty = $24,
            is_collection_guaranty = $25,
            is_continuing = $26,
            revolving_guaranty = $27,
            waivers_of_defenses = $28,
            suretyship_defenses_waived = $29,
            notice_of_default_waived = $30,
            notice_of_acceptance_waived = $31,
            subrogation_rights = $32,
            subrogation_subordinated = $33,
            subordination_provisions = $34,
            secured_guaranty = $35,
            guarantor_security_description = $36,
            guarantor_covenants = $37,
            release_conditions = $38,
            automatic_release_trigger = $39,
            partial_release_permitted = $40,
            effective_date = $41,
            termination_date = $42,
            governing_law = $43,
            enforceability_opinion_received = $44,
            opinion_reference = $45,
            corporate_authority_documents = $46,
            guaranty_status = $47,
            claims_filed = $48,
            amount_claimed = $49,
            amount_recovered = $50,
            notes = $51,
            metadata = $52,
            updated_by = $53
        WHERE id = $1
        RETURNING
            id, document_id, credit_agreement_id, doc_hash, guarantor_name, guarantor_type,
            guarantor_jurisdiction, guarantor_relationship, is_joint_guaranty, other_guarantors,
            beneficiary, guarantee_type, coverage_scope, covers_principal, covers_interest,
            covers_fees, covers_expenses, covers_amendments, is_capped, cap_amount, cap_currency,
            cap_type, cap_formula, is_payment_guaranty, is_collection_guaranty, is_continuing,
            revolving_guaranty, waivers_of_defenses, suretyship_defenses_waived,
            notice_of_default_waived, notice_of_acceptance_waived, subrogation_rights,
            subrogation_subordinated, subordination_provisions, secured_guaranty,
            guarantor_security_description, guarantor_covenants, release_conditions,
            automatic_release_trigger, partial_release_permitted, effective_date,
            termination_date, governing_law, enforceability_opinion_received, opinion_reference,
            corporate_authority_documents, guaranty_status, claims_filed, amount_claimed,
            amount_recovered, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.doc_hash.unwrap_or(existing.doc_hash),
        req.guarantor_name.unwrap_or(existing.guarantor_name),
        req.guarantor_type.or(existing.guarantor_type),
        req.guarantor_jurisdiction
            .or(existing.guarantor_jurisdiction),
        req.guarantor_relationship
            .or(existing.guarantor_relationship),
        req.is_joint_guaranty.or(existing.is_joint_guaranty),
        other_guarantors.as_deref(),
        req.beneficiary.unwrap_or(existing.beneficiary),
        req.guarantee_type.unwrap_or(existing.guarantee_type),
        req.coverage_scope.unwrap_or(existing.coverage_scope),
        req.covers_principal.or(existing.covers_principal),
        req.covers_interest.or(existing.covers_interest),
        req.covers_fees.or(existing.covers_fees),
        req.covers_expenses.or(existing.covers_expenses),
        req.covers_amendments.or(existing.covers_amendments),
        req.is_capped.or(existing.is_capped),
        req.cap_amount.or(existing.cap_amount),
        req.cap_currency.or(existing.cap_currency),
        req.cap_type.or(existing.cap_type),
        req.cap_formula.or(existing.cap_formula),
        req.is_payment_guaranty.or(existing.is_payment_guaranty),
        req.is_collection_guaranty
            .or(existing.is_collection_guaranty),
        req.is_continuing.or(existing.is_continuing),
        req.revolving_guaranty.or(existing.revolving_guaranty),
        req.waivers_of_defenses.or(existing.waivers_of_defenses),
        req.suretyship_defenses_waived
            .or(existing.suretyship_defenses_waived),
        req.notice_of_default_waived
            .or(existing.notice_of_default_waived),
        req.notice_of_acceptance_waived
            .or(existing.notice_of_acceptance_waived),
        req.subrogation_rights.or(existing.subrogation_rights),
        req.subrogation_subordinated
            .or(existing.subrogation_subordinated),
        req.subordination_provisions
            .or(existing.subordination_provisions),
        req.secured_guaranty.or(existing.secured_guaranty),
        req.guarantor_security_description
            .or(existing.guarantor_security_description),
        req.guarantor_covenants.or(existing.guarantor_covenants),
        req.release_conditions.or(existing.release_conditions),
        req.automatic_release_trigger
            .or(existing.automatic_release_trigger),
        req.partial_release_permitted
            .or(existing.partial_release_permitted),
        req.effective_date.unwrap_or(existing.effective_date),
        req.termination_date.or(existing.termination_date),
        req.governing_law.unwrap_or(existing.governing_law),
        req.enforceability_opinion_received
            .or(existing.enforceability_opinion_received),
        req.opinion_reference.or(existing.opinion_reference),
        req.corporate_authority_documents
            .or(existing.corporate_authority_documents),
        req.guaranty_status.or(existing.guaranty_status),
        req.claims_filed.or(existing.claims_filed),
        req.amount_claimed.or(existing.amount_claimed),
        req.amount_recovered.or(existing.amount_recovered),
        req.notes.or(existing.notes),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete guaranty
#[instrument(skip(pool))]
pub async fn delete_guaranty(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM guaranties WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple guaranties in a batch
#[instrument(skip(pool, requests))]
pub async fn create_guaranties_batch(
    pool: &PgPool,
    requests: Vec<CreateGuarantyRequest>,
) -> Result<Vec<Result<Guaranty, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_guaranty(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
