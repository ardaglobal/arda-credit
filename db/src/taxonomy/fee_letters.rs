//! Fee Letters Database Operations
//!
//! Provides CRUD operations for fee letter documents.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Fee Letter record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FeeLetter {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_fee_letter: String,
    pub fee_components: serde_json::Value,
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
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a fee letter
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateFeeLetterRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_fee_letter: String,
    pub fee_components: Option<serde_json::Value>,
    pub confidentiality_flag: Option<bool>,
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
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a fee letter
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateFeeLetterRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash_fee_letter: Option<String>,
    pub fee_components: Option<serde_json::Value>,
    pub confidentiality_flag: Option<bool>,
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
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new fee letter
#[instrument(skip(pool))]
pub async fn create_fee_letter(
    pool: &PgPool,
    req: CreateFeeLetterRequest,
) -> Result<FeeLetter, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        FeeLetter,
        r#"
        INSERT INTO fee_letters (
            id, document_id, credit_agreement_id, doc_hash_fee_letter, fee_components,
            confidentiality_flag, upfront_fee_pct, original_issue_discount_pct,
            unused_commitment_fee_bps, agency_fee_amount, amendment_fee_amount,
            fee_payment_schedule, fee_calculation_basis, effective_date, expiry_date,
            notes, metadata, created_at, updated_at, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
        )
        RETURNING
            id, document_id, credit_agreement_id, doc_hash_fee_letter, fee_components,
            confidentiality_flag, upfront_fee_pct, original_issue_discount_pct,
            unused_commitment_fee_bps, agency_fee_amount, amendment_fee_amount,
            fee_payment_schedule, fee_calculation_basis, effective_date, expiry_date,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.doc_hash_fee_letter,
        req.fee_components.unwrap_or(serde_json::json!({})),
        req.confidentiality_flag.unwrap_or(true),
        req.upfront_fee_pct,
        req.original_issue_discount_pct,
        req.unused_commitment_fee_bps,
        req.agency_fee_amount,
        req.amendment_fee_amount,
        req.fee_payment_schedule,
        req.fee_calculation_basis,
        req.effective_date,
        req.expiry_date,
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

/// Get fee letter by ID
#[instrument(skip(pool))]
pub async fn get_fee_letter_by_id(pool: &PgPool, id: Uuid) -> Result<Option<FeeLetter>, DbError> {
    let record = sqlx::query_as!(
        FeeLetter,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash_fee_letter, fee_components,
            confidentiality_flag, upfront_fee_pct, original_issue_discount_pct,
            unused_commitment_fee_bps, agency_fee_amount, amendment_fee_amount,
            fee_payment_schedule, fee_calculation_basis, effective_date, expiry_date,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM fee_letters WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get fee letter by document ID
#[instrument(skip(pool))]
pub async fn get_fee_letter_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<FeeLetter>, DbError> {
    let record = sqlx::query_as!(
        FeeLetter,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash_fee_letter, fee_components,
            confidentiality_flag, upfront_fee_pct, original_issue_discount_pct,
            unused_commitment_fee_bps, agency_fee_amount, amendment_fee_amount,
            fee_payment_schedule, fee_calculation_basis, effective_date, expiry_date,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM fee_letters WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List fee letters with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_fee_letters(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<FeeLetter>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    if credit_agreement_id.is_some() {
        where_clauses.push("credit_agreement_id = $3");
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
        Some("expiry_date") => "expiry_date",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!("SELECT COUNT(*) as count FROM fee_letters {}", where_clause);

    let total: i64 = sqlx::query_scalar(&count_query)
        .bind(&credit_agreement_id)
        .fetch_one(pool)
        .await?;

    // Get records
    let query = format!(
        "SELECT * FROM fee_letters {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let records = sqlx::query_as::<_, FeeLetter>(&query)
        .bind(limit)
        .bind(offset)
        .bind(&credit_agreement_id)
        .fetch_all(pool)
        .await?;

    Ok((records, total))
}

/// Update fee letter
#[instrument(skip(pool))]
pub async fn update_fee_letter(
    pool: &PgPool,
    id: Uuid,
    req: UpdateFeeLetterRequest,
    updated_by: Option<Uuid>,
) -> Result<FeeLetter, DbError> {
    // Get existing record
    let existing = get_fee_letter_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Fee letter {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        FeeLetter,
        r#"
        UPDATE fee_letters SET
            document_id = $2,
            credit_agreement_id = $3,
            doc_hash_fee_letter = $4,
            fee_components = $5,
            confidentiality_flag = $6,
            upfront_fee_pct = $7,
            original_issue_discount_pct = $8,
            unused_commitment_fee_bps = $9,
            agency_fee_amount = $10,
            amendment_fee_amount = $11,
            fee_payment_schedule = $12,
            fee_calculation_basis = $13,
            effective_date = $14,
            expiry_date = $15,
            notes = $16,
            metadata = $17,
            updated_by = $18
        WHERE id = $1
        RETURNING
            id, document_id, credit_agreement_id, doc_hash_fee_letter, fee_components,
            confidentiality_flag, upfront_fee_pct, original_issue_discount_pct,
            unused_commitment_fee_bps, agency_fee_amount, amendment_fee_amount,
            fee_payment_schedule, fee_calculation_basis, effective_date, expiry_date,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.doc_hash_fee_letter
            .unwrap_or(existing.doc_hash_fee_letter),
        req.fee_components.unwrap_or(existing.fee_components),
        req.confidentiality_flag
            .unwrap_or(existing.confidentiality_flag),
        req.upfront_fee_pct.or(existing.upfront_fee_pct),
        req.original_issue_discount_pct
            .or(existing.original_issue_discount_pct),
        req.unused_commitment_fee_bps
            .or(existing.unused_commitment_fee_bps),
        req.agency_fee_amount.or(existing.agency_fee_amount),
        req.amendment_fee_amount.or(existing.amendment_fee_amount),
        req.fee_payment_schedule.or(existing.fee_payment_schedule),
        req.fee_calculation_basis.or(existing.fee_calculation_basis),
        req.effective_date.or(existing.effective_date),
        req.expiry_date.or(existing.expiry_date),
        req.notes.or(existing.notes),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete fee letter
#[instrument(skip(pool))]
pub async fn delete_fee_letter(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM fee_letters WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple fee letters in a batch
#[instrument(skip(pool, requests))]
pub async fn create_fee_letters_batch(
    pool: &PgPool,
    requests: Vec<CreateFeeLetterRequest>,
) -> Result<Vec<Result<FeeLetter, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_fee_letter(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
