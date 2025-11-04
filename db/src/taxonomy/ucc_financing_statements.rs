//! UCC Financing Statements Database Operations
//!
//! Provides CRUD operations for UCC-1 financing statement filings.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// UCC Financing Statement record with full filing attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UccFinancingStatement {
    pub id: Uuid,
    pub document_id: String,
    pub security_agreement_id: Option<Uuid>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub file_number: String,
    pub filing_office: String,
    pub jurisdiction: String,
    pub file_datetime: DateTime<Utc>,
    pub expiration_date: Option<NaiveDate>,
    pub lapse_date: Option<NaiveDate>,
    pub filing_type: String,
    pub original_file_number: Option<String>,
    pub amends_file_number: Option<String>,
    pub debtor_name: String,
    pub debtor_type: Option<String>,
    pub debtor_jurisdiction: Option<String>,
    pub debtor_organization_id: Option<String>,
    pub debtor_address: Option<String>,
    pub secured_party: String,
    pub secured_party_address: Option<String>,
    pub additional_debtors: serde_json::Value,
    pub additional_secured_parties: serde_json::Value,
    pub collateral_description: String,
    pub collateral_desc_short: Option<String>,
    pub all_assets_indicator: bool,
    pub specific_collateral: bool,
    pub filing_status: String,
    pub continuation_filed: bool,
    pub continuation_file_number: Option<String>,
    pub continuation_date: Option<NaiveDate>,
    pub terminated: bool,
    pub termination_date: Option<NaiveDate>,
    pub termination_reason: Option<String>,
    pub is_agricultural_lien: bool,
    pub is_fixture_filing: bool,
    pub is_manufactured_home: bool,
    pub real_property_description: Option<String>,
    pub filing_fee: Option<Decimal>,
    pub search_verified: bool,
    pub search_verification_date: Option<NaiveDate>,
    pub search_results: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a UCC financing statement
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUccFinancingStatementRequest {
    pub document_id: String,
    pub security_agreement_id: Option<Uuid>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub file_number: String,
    pub filing_office: String,
    pub jurisdiction: String,
    pub file_datetime: DateTime<Utc>,
    pub expiration_date: Option<NaiveDate>,
    pub lapse_date: Option<NaiveDate>,
    pub filing_type: Option<String>,
    pub original_file_number: Option<String>,
    pub amends_file_number: Option<String>,
    pub debtor_name: String,
    pub debtor_type: Option<String>,
    pub debtor_jurisdiction: Option<String>,
    pub debtor_organization_id: Option<String>,
    pub debtor_address: Option<String>,
    pub secured_party: String,
    pub secured_party_address: Option<String>,
    pub additional_debtors: Option<serde_json::Value>,
    pub additional_secured_parties: Option<serde_json::Value>,
    pub collateral_description: String,
    pub collateral_desc_short: Option<String>,
    pub all_assets_indicator: Option<bool>,
    pub specific_collateral: Option<bool>,
    pub filing_status: Option<String>,
    pub continuation_filed: Option<bool>,
    pub continuation_file_number: Option<String>,
    pub continuation_date: Option<NaiveDate>,
    pub terminated: Option<bool>,
    pub termination_date: Option<NaiveDate>,
    pub termination_reason: Option<String>,
    pub is_agricultural_lien: Option<bool>,
    pub is_fixture_filing: Option<bool>,
    pub is_manufactured_home: Option<bool>,
    pub real_property_description: Option<String>,
    pub filing_fee: Option<Decimal>,
    pub search_verified: Option<bool>,
    pub search_verification_date: Option<NaiveDate>,
    pub search_results: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a UCC financing statement
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUccFinancingStatementRequest {
    pub document_id: Option<String>,
    pub security_agreement_id: Option<Uuid>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: Option<String>,
    pub file_number: Option<String>,
    pub filing_office: Option<String>,
    pub jurisdiction: Option<String>,
    pub file_datetime: Option<DateTime<Utc>>,
    pub expiration_date: Option<NaiveDate>,
    pub lapse_date: Option<NaiveDate>,
    pub filing_type: Option<String>,
    pub original_file_number: Option<String>,
    pub amends_file_number: Option<String>,
    pub debtor_name: Option<String>,
    pub debtor_type: Option<String>,
    pub debtor_jurisdiction: Option<String>,
    pub debtor_organization_id: Option<String>,
    pub debtor_address: Option<String>,
    pub secured_party: Option<String>,
    pub secured_party_address: Option<String>,
    pub additional_debtors: Option<serde_json::Value>,
    pub additional_secured_parties: Option<serde_json::Value>,
    pub collateral_description: Option<String>,
    pub collateral_desc_short: Option<String>,
    pub all_assets_indicator: Option<bool>,
    pub specific_collateral: Option<bool>,
    pub filing_status: Option<String>,
    pub continuation_filed: Option<bool>,
    pub continuation_file_number: Option<String>,
    pub continuation_date: Option<NaiveDate>,
    pub terminated: Option<bool>,
    pub termination_date: Option<NaiveDate>,
    pub termination_reason: Option<String>,
    pub is_agricultural_lien: Option<bool>,
    pub is_fixture_filing: Option<bool>,
    pub is_manufactured_home: Option<bool>,
    pub real_property_description: Option<String>,
    pub filing_fee: Option<Decimal>,
    pub search_verified: Option<bool>,
    pub search_verification_date: Option<NaiveDate>,
    pub search_results: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new UCC financing statement
#[instrument(skip(pool))]
pub async fn create_ucc_financing_statement(
    pool: &PgPool,
    req: CreateUccFinancingStatementRequest,
) -> Result<UccFinancingStatement, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        UccFinancingStatement,
        r#"
        INSERT INTO ucc_financing_statements (
            id, document_id, security_agreement_id, credit_agreement_id, doc_hash,
            file_number, filing_office, jurisdiction, file_datetime, expiration_date,
            lapse_date, filing_type, original_file_number, amends_file_number,
            debtor_name, debtor_type, debtor_jurisdiction, debtor_organization_id,
            debtor_address, secured_party, secured_party_address,
            additional_debtors, additional_secured_parties,
            collateral_description, collateral_desc_short,
            all_assets_indicator, specific_collateral, filing_status,
            continuation_filed, continuation_file_number, continuation_date,
            terminated, termination_date, termination_reason,
            is_agricultural_lien, is_fixture_filing, is_manufactured_home,
            real_property_description, filing_fee,
            search_verified, search_verification_date, search_results,
            notes, metadata, created_at, updated_at, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
            $31, $32, $33, $34, $35, $36, $37, $38, $39, $40,
            $41, $42, $43, $44, $45, $46, $47
        )
        RETURNING
            id, document_id, security_agreement_id, credit_agreement_id, doc_hash,
            file_number, filing_office, jurisdiction,
            file_datetime as "file_datetime!", expiration_date, lapse_date,
            filing_type, original_file_number, amends_file_number,
            debtor_name, debtor_type, debtor_jurisdiction, debtor_organization_id,
            debtor_address, secured_party, secured_party_address,
            additional_debtors, additional_secured_parties,
            collateral_description, collateral_desc_short,
            all_assets_indicator as "all_assets_indicator!",
            specific_collateral as "specific_collateral!",
            filing_status,
            continuation_filed as "continuation_filed!",
            continuation_file_number, continuation_date,
            terminated as "terminated!",
            termination_date, termination_reason,
            is_agricultural_lien as "is_agricultural_lien!",
            is_fixture_filing as "is_fixture_filing!",
            is_manufactured_home as "is_manufactured_home!",
            real_property_description, filing_fee,
            search_verified as "search_verified!",
            search_verification_date, search_results,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.security_agreement_id,
        req.credit_agreement_id,
        req.doc_hash,
        req.file_number,
        req.filing_office,
        req.jurisdiction,
        req.file_datetime,
        req.expiration_date,
        req.lapse_date,
        req.filing_type.unwrap_or_else(|| "initial".to_string()),
        req.original_file_number,
        req.amends_file_number,
        req.debtor_name,
        req.debtor_type,
        req.debtor_jurisdiction,
        req.debtor_organization_id,
        req.debtor_address,
        req.secured_party,
        req.secured_party_address,
        req.additional_debtors.unwrap_or(serde_json::json!([])),
        req.additional_secured_parties
            .unwrap_or(serde_json::json!([])),
        req.collateral_description,
        req.collateral_desc_short,
        req.all_assets_indicator.unwrap_or(false),
        req.specific_collateral.unwrap_or(true),
        req.filing_status.unwrap_or_else(|| "active".to_string()),
        req.continuation_filed.unwrap_or(false),
        req.continuation_file_number,
        req.continuation_date,
        req.terminated.unwrap_or(false),
        req.termination_date,
        req.termination_reason,
        req.is_agricultural_lien.unwrap_or(false),
        req.is_fixture_filing.unwrap_or(false),
        req.is_manufactured_home.unwrap_or(false),
        req.real_property_description,
        req.filing_fee,
        req.search_verified.unwrap_or(false),
        req.search_verification_date,
        req.search_results,
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

/// Get UCC financing statement by ID
#[instrument(skip(pool))]
pub async fn get_ucc_financing_statement_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<UccFinancingStatement>, DbError> {
    let record = sqlx::query_as!(
        UccFinancingStatement,
        r#"
        SELECT
            id, document_id, security_agreement_id, credit_agreement_id, doc_hash,
            file_number, filing_office, jurisdiction,
            file_datetime as "file_datetime!", expiration_date, lapse_date,
            filing_type, original_file_number, amends_file_number,
            debtor_name, debtor_type, debtor_jurisdiction, debtor_organization_id,
            debtor_address, secured_party, secured_party_address,
            additional_debtors, additional_secured_parties,
            collateral_description, collateral_desc_short,
            all_assets_indicator as "all_assets_indicator!",
            specific_collateral as "specific_collateral!",
            filing_status,
            continuation_filed as "continuation_filed!",
            continuation_file_number, continuation_date,
            terminated as "terminated!",
            termination_date, termination_reason,
            is_agricultural_lien as "is_agricultural_lien!",
            is_fixture_filing as "is_fixture_filing!",
            is_manufactured_home as "is_manufactured_home!",
            real_property_description, filing_fee,
            search_verified as "search_verified!",
            search_verification_date, search_results,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM ucc_financing_statements WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get UCC financing statement by document ID
#[instrument(skip(pool))]
pub async fn get_ucc_financing_statement_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<UccFinancingStatement>, DbError> {
    let record = sqlx::query_as!(
        UccFinancingStatement,
        r#"
        SELECT
            id, document_id, security_agreement_id, credit_agreement_id, doc_hash,
            file_number, filing_office, jurisdiction,
            file_datetime as "file_datetime!", expiration_date, lapse_date,
            filing_type, original_file_number, amends_file_number,
            debtor_name, debtor_type, debtor_jurisdiction, debtor_organization_id,
            debtor_address, secured_party, secured_party_address,
            additional_debtors, additional_secured_parties,
            collateral_description, collateral_desc_short,
            all_assets_indicator as "all_assets_indicator!",
            specific_collateral as "specific_collateral!",
            filing_status,
            continuation_filed as "continuation_filed!",
            continuation_file_number, continuation_date,
            terminated as "terminated!",
            termination_date, termination_reason,
            is_agricultural_lien as "is_agricultural_lien!",
            is_fixture_filing as "is_fixture_filing!",
            is_manufactured_home as "is_manufactured_home!",
            real_property_description, filing_fee,
            search_verified as "search_verified!",
            search_verification_date, search_results,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM ucc_financing_statements WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List UCC financing statements with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_ucc_financing_statements(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    security_agreement_id: Option<Uuid>,
    jurisdiction: Option<String>,
    filing_status: Option<String>,
    filing_type: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<UccFinancingStatement>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_index = 3; // Start after limit and offset

    if security_agreement_id.is_some() {
        where_clauses.push(format!("security_agreement_id = ${}", param_index));
        param_index += 1;
    }
    if jurisdiction.is_some() {
        where_clauses.push(format!("jurisdiction = ${}", param_index));
        param_index += 1;
    }
    if filing_status.is_some() {
        where_clauses.push(format!("filing_status = ${}", param_index));
        param_index += 1;
    }
    if filing_type.is_some() {
        where_clauses.push(format!("filing_type = ${}", param_index));
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("file_datetime") => "file_datetime",
        Some("expiration_date") => "expiration_date",
        Some("jurisdiction") => "jurisdiction",
        Some("filing_status") => "filing_status",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM ucc_financing_statements {}",
        where_clause
    );

    let mut count_query = sqlx::query_scalar(&count_query);
    count_query = count_query.bind(limit).bind(offset);
    if let Some(id) = security_agreement_id {
        count_query = count_query.bind(id);
    }
    if let Some(j) = &jurisdiction {
        count_query = count_query.bind(j);
    }
    if let Some(fs) = &filing_status {
        count_query = count_query.bind(fs);
    }
    if let Some(ft) = &filing_type {
        count_query = count_query.bind(ft);
    }

    let total: i64 = count_query.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM ucc_financing_statements {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut records_query = sqlx::query_as::<_, UccFinancingStatement>(&query);
    records_query = records_query.bind(limit).bind(offset);
    if let Some(id) = security_agreement_id {
        records_query = records_query.bind(id);
    }
    if let Some(j) = &jurisdiction {
        records_query = records_query.bind(j);
    }
    if let Some(fs) = &filing_status {
        records_query = records_query.bind(fs);
    }
    if let Some(ft) = &filing_type {
        records_query = records_query.bind(ft);
    }

    let records = records_query.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update UCC financing statement
#[instrument(skip(pool))]
pub async fn update_ucc_financing_statement(
    pool: &PgPool,
    id: Uuid,
    req: UpdateUccFinancingStatementRequest,
    updated_by: Option<Uuid>,
) -> Result<UccFinancingStatement, DbError> {
    // Get existing record
    let existing = get_ucc_financing_statement_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("UCC financing statement {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        UccFinancingStatement,
        r#"
        UPDATE ucc_financing_statements SET
            document_id = $2,
            security_agreement_id = $3,
            credit_agreement_id = $4,
            doc_hash = $5,
            file_number = $6,
            filing_office = $7,
            jurisdiction = $8,
            file_datetime = $9,
            expiration_date = $10,
            lapse_date = $11,
            filing_type = $12,
            original_file_number = $13,
            amends_file_number = $14,
            debtor_name = $15,
            debtor_type = $16,
            debtor_jurisdiction = $17,
            debtor_organization_id = $18,
            debtor_address = $19,
            secured_party = $20,
            secured_party_address = $21,
            additional_debtors = $22,
            additional_secured_parties = $23,
            collateral_description = $24,
            collateral_desc_short = $25,
            all_assets_indicator = $26,
            specific_collateral = $27,
            filing_status = $28,
            continuation_filed = $29,
            continuation_file_number = $30,
            continuation_date = $31,
            terminated = $32,
            termination_date = $33,
            termination_reason = $34,
            is_agricultural_lien = $35,
            is_fixture_filing = $36,
            is_manufactured_home = $37,
            real_property_description = $38,
            filing_fee = $39,
            search_verified = $40,
            search_verification_date = $41,
            search_results = $42,
            notes = $43,
            metadata = $44,
            updated_by = $45
        WHERE id = $1
        RETURNING
            id, document_id, security_agreement_id, credit_agreement_id, doc_hash,
            file_number, filing_office, jurisdiction,
            file_datetime as "file_datetime!", expiration_date, lapse_date,
            filing_type, original_file_number, amends_file_number,
            debtor_name, debtor_type, debtor_jurisdiction, debtor_organization_id,
            debtor_address, secured_party, secured_party_address,
            additional_debtors, additional_secured_parties,
            collateral_description, collateral_desc_short,
            all_assets_indicator as "all_assets_indicator!",
            specific_collateral as "specific_collateral!",
            filing_status,
            continuation_filed as "continuation_filed!",
            continuation_file_number, continuation_date,
            terminated as "terminated!",
            termination_date, termination_reason,
            is_agricultural_lien as "is_agricultural_lien!",
            is_fixture_filing as "is_fixture_filing!",
            is_manufactured_home as "is_manufactured_home!",
            real_property_description, filing_fee,
            search_verified as "search_verified!",
            search_verification_date, search_results,
            notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.security_agreement_id.or(existing.security_agreement_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.doc_hash.unwrap_or(existing.doc_hash),
        req.file_number.unwrap_or(existing.file_number),
        req.filing_office.unwrap_or(existing.filing_office),
        req.jurisdiction.unwrap_or(existing.jurisdiction),
        req.file_datetime.unwrap_or(existing.file_datetime),
        req.expiration_date.or(existing.expiration_date),
        req.lapse_date.or(existing.lapse_date),
        req.filing_type.unwrap_or(existing.filing_type),
        req.original_file_number.or(existing.original_file_number),
        req.amends_file_number.or(existing.amends_file_number),
        req.debtor_name.unwrap_or(existing.debtor_name),
        req.debtor_type.or(existing.debtor_type),
        req.debtor_jurisdiction.or(existing.debtor_jurisdiction),
        req.debtor_organization_id
            .or(existing.debtor_organization_id),
        req.debtor_address.or(existing.debtor_address),
        req.secured_party.unwrap_or(existing.secured_party),
        req.secured_party_address.or(existing.secured_party_address),
        req.additional_debtors
            .unwrap_or(existing.additional_debtors),
        req.additional_secured_parties
            .unwrap_or(existing.additional_secured_parties),
        req.collateral_description
            .unwrap_or(existing.collateral_description),
        req.collateral_desc_short.or(existing.collateral_desc_short),
        req.all_assets_indicator
            .unwrap_or(existing.all_assets_indicator),
        req.specific_collateral
            .unwrap_or(existing.specific_collateral),
        req.filing_status.unwrap_or(existing.filing_status),
        req.continuation_filed
            .unwrap_or(existing.continuation_filed),
        req.continuation_file_number
            .or(existing.continuation_file_number),
        req.continuation_date.or(existing.continuation_date),
        req.terminated.unwrap_or(existing.terminated),
        req.termination_date.or(existing.termination_date),
        req.termination_reason.or(existing.termination_reason),
        req.is_agricultural_lien
            .unwrap_or(existing.is_agricultural_lien),
        req.is_fixture_filing.unwrap_or(existing.is_fixture_filing),
        req.is_manufactured_home
            .unwrap_or(existing.is_manufactured_home),
        req.real_property_description
            .or(existing.real_property_description),
        req.filing_fee.or(existing.filing_fee),
        req.search_verified.unwrap_or(existing.search_verified),
        req.search_verification_date
            .or(existing.search_verification_date),
        req.search_results.or(existing.search_results),
        req.notes.or(existing.notes),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete UCC financing statement
#[instrument(skip(pool))]
pub async fn delete_ucc_financing_statement(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM ucc_financing_statements WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple UCC financing statements in a batch
#[instrument(skip(pool, requests))]
pub async fn create_ucc_financing_statements_batch(
    pool: &PgPool,
    requests: Vec<CreateUccFinancingStatementRequest>,
) -> Result<Vec<Result<UccFinancingStatement, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_ucc_financing_statement(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
