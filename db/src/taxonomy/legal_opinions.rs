//! Legal Opinions Database Operations
//!
//! Provides CRUD operations for legal opinion documents.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Legal Opinion record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LegalOpinion {
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
    pub beneficiaries: Option<Vec<String>>,
    pub opinion_date: NaiveDate,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: bool,
    pub disclosure_schedules_attached: Option<bool>,
    pub bring_down_opinion_id: Option<Uuid>,
    pub is_bring_down: Option<bool>,
    pub original_opinion_id: Option<Uuid>,
    pub coverage_status: Option<String>,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a legal opinion
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLegalOpinionRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_legal_opinion: Vec<String>,
    pub opinion_firms: Vec<String>,
    pub lead_opinion_firm: Option<String>,
    pub scopes: Vec<String>,
    pub governing_laws: Vec<String>,
    pub primary_governing_law: Option<String>,
    pub reliance_language_present: Option<bool>,
    pub beneficiaries: Option<Vec<String>>,
    pub opinion_date: NaiveDate,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: Option<bool>,
    pub disclosure_schedules_attached: Option<bool>,
    pub bring_down_opinion_id: Option<Uuid>,
    pub is_bring_down: Option<bool>,
    pub original_opinion_id: Option<Uuid>,
    pub coverage_status: Option<String>,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a legal opinion
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLegalOpinionRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_legal_opinion: Option<Vec<String>>,
    pub opinion_firms: Option<Vec<String>>,
    pub lead_opinion_firm: Option<String>,
    pub scopes: Option<Vec<String>>,
    pub governing_laws: Option<Vec<String>>,
    pub primary_governing_law: Option<String>,
    pub reliance_language_present: Option<bool>,
    pub beneficiaries: Option<Vec<String>>,
    pub opinion_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub limitations: Option<String>,
    pub assumptions: Option<String>,
    pub full_opinion_delivered: Option<bool>,
    pub disclosure_schedules_attached: Option<bool>,
    pub bring_down_opinion_id: Option<Uuid>,
    pub is_bring_down: Option<bool>,
    pub original_opinion_id: Option<Uuid>,
    pub coverage_status: Option<String>,
    pub gaps_or_qualifications: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new legal opinion
#[instrument(skip(pool))]
pub async fn create_legal_opinion(
    pool: &PgPool,
    req: CreateLegalOpinionRequest,
) -> Result<LegalOpinion, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        LegalOpinion,
        r#"
        INSERT INTO legal_opinions (
            id, document_id, credit_agreement_id, doc_hash_legal_opinion, opinion_firms,
            lead_opinion_firm, scopes, governing_laws, primary_governing_law,
            reliance_language_present, beneficiaries, opinion_date, effective_date,
            limitations, assumptions, full_opinion_delivered, disclosure_schedules_attached,
            bring_down_opinion_id, is_bring_down, original_opinion_id, coverage_status,
            gaps_or_qualifications, notes, metadata, created_at, updated_at, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17,
            $18, $19, $20, $21, $22, $23, $24, $25, $26, $27
        )
        RETURNING
            id, document_id, credit_agreement_id, doc_hash_legal_opinion, opinion_firms,
            lead_opinion_firm, scopes, governing_laws, primary_governing_law,
            reliance_language_present, beneficiaries, opinion_date, effective_date,
            limitations, assumptions, full_opinion_delivered, disclosure_schedules_attached,
            bring_down_opinion_id, is_bring_down, original_opinion_id, coverage_status,
            gaps_or_qualifications, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        &req.doc_hash_legal_opinion,
        &req.opinion_firms,
        req.lead_opinion_firm,
        &req.scopes,
        &req.governing_laws,
        req.primary_governing_law,
        req.reliance_language_present.unwrap_or(false),
        req.beneficiaries.as_deref(),
        req.opinion_date,
        req.effective_date,
        req.limitations,
        req.assumptions,
        req.full_opinion_delivered.unwrap_or(false),
        req.disclosure_schedules_attached,
        req.bring_down_opinion_id,
        req.is_bring_down,
        req.original_opinion_id,
        req.coverage_status
            .unwrap_or_else(|| "complete".to_string()),
        req.gaps_or_qualifications,
        req.notes,
        req.metadata,
        now,
        now,
        req.created_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get legal opinion by ID
#[instrument(skip(pool))]
pub async fn get_legal_opinion_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<LegalOpinion>, DbError> {
    let record = sqlx::query_as!(
        LegalOpinion,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash_legal_opinion, opinion_firms,
            lead_opinion_firm, scopes, governing_laws, primary_governing_law,
            reliance_language_present, beneficiaries, opinion_date, effective_date,
            limitations, assumptions, full_opinion_delivered, disclosure_schedules_attached,
            bring_down_opinion_id, is_bring_down, original_opinion_id, coverage_status,
            gaps_or_qualifications, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM legal_opinions WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get legal opinion by document ID
#[instrument(skip(pool))]
pub async fn get_legal_opinion_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<LegalOpinion>, DbError> {
    let record = sqlx::query_as!(
        LegalOpinion,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash_legal_opinion, opinion_firms,
            lead_opinion_firm, scopes, governing_laws, primary_governing_law,
            reliance_language_present, beneficiaries, opinion_date, effective_date,
            limitations, assumptions, full_opinion_delivered, disclosure_schedules_attached,
            bring_down_opinion_id, is_bring_down, original_opinion_id, coverage_status,
            gaps_or_qualifications, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM legal_opinions WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List legal opinions with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_legal_opinions(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    coverage_status: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<LegalOpinion>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    if credit_agreement_id.is_some() {
        where_clauses.push("credit_agreement_id = $3");
    }
    if coverage_status.is_some() {
        where_clauses.push("coverage_status = $4");
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("opinion_date") => "opinion_date",
        Some("coverage_status") => "coverage_status",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM legal_opinions {}",
        where_clause
    );

    let total: i64 = sqlx::query_scalar(&count_query)
        .bind(&credit_agreement_id)
        .bind(&coverage_status)
        .fetch_one(pool)
        .await?;

    // Get records
    let query = format!(
        "SELECT * FROM legal_opinions {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let records = sqlx::query_as::<_, LegalOpinion>(&query)
        .bind(limit)
        .bind(offset)
        .bind(&credit_agreement_id)
        .bind(&coverage_status)
        .fetch_all(pool)
        .await?;

    Ok((records, total))
}

/// Update legal opinion
#[instrument(skip(pool))]
pub async fn update_legal_opinion(
    pool: &PgPool,
    id: Uuid,
    req: UpdateLegalOpinionRequest,
    updated_by: Option<Uuid>,
) -> Result<LegalOpinion, DbError> {
    // Get existing record
    let existing = get_legal_opinion_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Legal opinion {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        LegalOpinion,
        r#"
        UPDATE legal_opinions SET
            document_id = $2,
            credit_agreement_id = $3,
            doc_hash_legal_opinion = $4,
            opinion_firms = $5,
            lead_opinion_firm = $6,
            scopes = $7,
            governing_laws = $8,
            primary_governing_law = $9,
            reliance_language_present = $10,
            beneficiaries = $11,
            opinion_date = $12,
            effective_date = $13,
            limitations = $14,
            assumptions = $15,
            full_opinion_delivered = $16,
            disclosure_schedules_attached = $17,
            bring_down_opinion_id = $18,
            is_bring_down = $19,
            original_opinion_id = $20,
            coverage_status = $21,
            gaps_or_qualifications = $22,
            notes = $23,
            metadata = $24,
            updated_by = $25
        WHERE id = $1
        RETURNING
            id, document_id, credit_agreement_id, doc_hash_legal_opinion, opinion_firms,
            lead_opinion_firm, scopes, governing_laws, primary_governing_law,
            reliance_language_present, beneficiaries, opinion_date, effective_date,
            limitations, assumptions, full_opinion_delivered, disclosure_schedules_attached,
            bring_down_opinion_id, is_bring_down, original_opinion_id, coverage_status,
            gaps_or_qualifications, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.doc_hash_legal_opinion
            .as_deref()
            .unwrap_or(&existing.doc_hash_legal_opinion),
        req.opinion_firms
            .as_deref()
            .unwrap_or(&existing.opinion_firms),
        req.lead_opinion_firm.or(existing.lead_opinion_firm),
        req.scopes.as_deref().unwrap_or(&existing.scopes),
        req.governing_laws
            .as_deref()
            .unwrap_or(&existing.governing_laws),
        req.primary_governing_law.or(existing.primary_governing_law),
        req.reliance_language_present
            .unwrap_or(existing.reliance_language_present),
        req.beneficiaries
            .as_deref()
            .or(existing.beneficiaries.as_deref()),
        req.opinion_date.unwrap_or(existing.opinion_date),
        req.effective_date.or(existing.effective_date),
        req.limitations.or(existing.limitations),
        req.assumptions.or(existing.assumptions),
        req.full_opinion_delivered
            .unwrap_or(existing.full_opinion_delivered),
        req.disclosure_schedules_attached
            .or(existing.disclosure_schedules_attached),
        req.bring_down_opinion_id.or(existing.bring_down_opinion_id),
        req.is_bring_down.or(existing.is_bring_down),
        req.original_opinion_id.or(existing.original_opinion_id),
        req.coverage_status.or(existing.coverage_status),
        req.gaps_or_qualifications
            .or(existing.gaps_or_qualifications),
        req.notes.or(existing.notes),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete legal opinion
#[instrument(skip(pool))]
pub async fn delete_legal_opinion(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM legal_opinions WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple legal opinions in a batch
#[instrument(skip(pool, requests))]
pub async fn create_legal_opinions_batch(
    pool: &PgPool,
    requests: Vec<CreateLegalOpinionRequest>,
) -> Result<Vec<Result<LegalOpinion, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_legal_opinion(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
