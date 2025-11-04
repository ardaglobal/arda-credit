//! SSI Records Database Operations
//!
//! Provides CRUD operations for Standard Settlement Instructions (SSI) documents.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// SSI Record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SsiRecord {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub doc_hash: String,
    pub ssi_type: Option<String>, // DEFAULT 'full_bilateral'

    // Seller SSI Details
    pub seller_ssi_id: String,
    pub seller_beneficiary_name: String,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_swift_code: Option<String>,
    pub seller_aba_routing: Option<String>,
    pub seller_iban: Option<String>,
    pub seller_account_number: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_bank_address: Option<String>,
    pub seller_intermediary_bank: Option<String>,
    pub seller_intermediary_swift: Option<String>,
    pub seller_custody_type: Option<String>, // DEFAULT 'bank'
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_address: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub seller_wallet_type: Option<String>,

    // Buyer SSI Details
    pub buyer_ssi_id: String,
    pub buyer_beneficiary_name: String,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_swift_code: Option<String>,
    pub buyer_aba_routing: Option<String>,
    pub buyer_iban: Option<String>,
    pub buyer_account_number: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_bank_address: Option<String>,
    pub buyer_intermediary_bank: Option<String>,
    pub buyer_intermediary_swift: Option<String>,
    pub buyer_custody_type: Option<String>, // DEFAULT 'bank'
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_address: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub buyer_wallet_type: Option<String>,

    // SSI Status & Validation
    pub seller_ssi_verified: Option<bool>, // DEFAULT FALSE
    pub seller_ssi_verified_at: Option<DateTime<Utc>>,
    pub seller_ssi_verified_by: Option<String>,
    pub buyer_ssi_verified: Option<bool>, // DEFAULT FALSE
    pub buyer_ssi_verified_at: Option<DateTime<Utc>>,
    pub buyer_ssi_verified_by: Option<String>,
    pub ssi_status: Option<String>, // DEFAULT 'draft'
    pub effective_date: NaiveDate,
    pub expiry_date: Option<NaiveDate>,
    pub last_used_date: Option<NaiveDate>,
    pub usage_count: Option<i32>, // DEFAULT 0

    // Compliance & Security
    pub sanctions_screened: Option<bool>, // DEFAULT FALSE
    pub sanctions_screen_date: Option<DateTime<Utc>>,
    pub sanctions_screen_result: Option<String>,
    pub aml_check_passed: Option<bool>, // DEFAULT FALSE
    pub aml_check_date: Option<DateTime<Utc>>,
    pub confidential: Option<bool>, // DEFAULT TRUE
    pub access_restricted_to: Option<Vec<String>>,

    // Metadata & Audit
    pub settlement_currency: Option<String>, // DEFAULT 'USD'
    pub settlement_method: Option<String>,   // DEFAULT 'wire'
    pub special_instructions: Option<String>,
    pub internal_notes: Option<String>,
    pub metadata: Option<serde_json::Value>, // DEFAULT '{}'
    pub created_at: Option<DateTime<Utc>>,   // DEFAULT CURRENT_TIMESTAMP
    pub updated_at: Option<DateTime<Utc>>,   // DEFAULT CURRENT_TIMESTAMP
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create an SSI record
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSsiRecordRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub doc_hash: String,
    pub ssi_type: Option<String>,

    // Seller SSI Details
    pub seller_ssi_id: String,
    pub seller_beneficiary_name: String,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_swift_code: Option<String>,
    pub seller_aba_routing: Option<String>,
    pub seller_iban: Option<String>,
    pub seller_account_number: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_bank_address: Option<String>,
    pub seller_intermediary_bank: Option<String>,
    pub seller_intermediary_swift: Option<String>,
    pub seller_custody_type: Option<String>,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_address: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub seller_wallet_type: Option<String>,

    // Buyer SSI Details
    pub buyer_ssi_id: String,
    pub buyer_beneficiary_name: String,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_swift_code: Option<String>,
    pub buyer_aba_routing: Option<String>,
    pub buyer_iban: Option<String>,
    pub buyer_account_number: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_bank_address: Option<String>,
    pub buyer_intermediary_bank: Option<String>,
    pub buyer_intermediary_swift: Option<String>,
    pub buyer_custody_type: Option<String>,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_address: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub buyer_wallet_type: Option<String>,

    // SSI Status & Validation
    pub seller_ssi_verified: Option<bool>,
    pub seller_ssi_verified_at: Option<DateTime<Utc>>,
    pub seller_ssi_verified_by: Option<String>,
    pub buyer_ssi_verified: Option<bool>,
    pub buyer_ssi_verified_at: Option<DateTime<Utc>>,
    pub buyer_ssi_verified_by: Option<String>,
    pub ssi_status: Option<String>,
    pub effective_date: NaiveDate,
    pub expiry_date: Option<NaiveDate>,
    pub last_used_date: Option<NaiveDate>,
    pub usage_count: Option<i32>,

    // Compliance & Security
    pub sanctions_screened: Option<bool>,
    pub sanctions_screen_date: Option<DateTime<Utc>>,
    pub sanctions_screen_result: Option<String>,
    pub aml_check_passed: Option<bool>,
    pub aml_check_date: Option<DateTime<Utc>>,
    pub confidential: Option<bool>,
    pub access_restricted_to: Option<Vec<String>>,

    // Metadata & Audit
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>,
    pub special_instructions: Option<String>,
    pub internal_notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update an SSI record
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSsiRecordRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub deal_id: Option<Uuid>,
    pub doc_hash: Option<String>,
    pub ssi_type: Option<String>,

    // Seller SSI Details
    pub seller_ssi_id: Option<String>,
    pub seller_beneficiary_name: Option<String>,
    pub seller_bank_name: Option<String>,
    pub seller_bank_identifier: Option<String>,
    pub seller_swift_code: Option<String>,
    pub seller_aba_routing: Option<String>,
    pub seller_iban: Option<String>,
    pub seller_account_number: Option<String>,
    pub seller_reference_text: Option<String>,
    pub seller_bank_address: Option<String>,
    pub seller_intermediary_bank: Option<String>,
    pub seller_intermediary_swift: Option<String>,
    pub seller_custody_type: Option<String>,
    pub seller_custodian_name: Option<String>,
    pub seller_custodian_account_ref: Option<String>,
    pub seller_wallet_address: Option<String>,
    pub seller_wallet_chain: Option<String>,
    pub seller_wallet_chain_id: Option<i32>,
    pub seller_wallet_type: Option<String>,

    // Buyer SSI Details
    pub buyer_ssi_id: Option<String>,
    pub buyer_beneficiary_name: Option<String>,
    pub buyer_bank_name: Option<String>,
    pub buyer_bank_identifier: Option<String>,
    pub buyer_swift_code: Option<String>,
    pub buyer_aba_routing: Option<String>,
    pub buyer_iban: Option<String>,
    pub buyer_account_number: Option<String>,
    pub buyer_reference_text: Option<String>,
    pub buyer_bank_address: Option<String>,
    pub buyer_intermediary_bank: Option<String>,
    pub buyer_intermediary_swift: Option<String>,
    pub buyer_custody_type: Option<String>,
    pub buyer_custodian_name: Option<String>,
    pub buyer_custodian_account_ref: Option<String>,
    pub buyer_wallet_address: Option<String>,
    pub buyer_wallet_chain: Option<String>,
    pub buyer_wallet_chain_id: Option<i32>,
    pub buyer_wallet_type: Option<String>,

    // SSI Status & Validation
    pub seller_ssi_verified: Option<bool>,
    pub seller_ssi_verified_at: Option<DateTime<Utc>>,
    pub seller_ssi_verified_by: Option<String>,
    pub buyer_ssi_verified: Option<bool>,
    pub buyer_ssi_verified_at: Option<DateTime<Utc>>,
    pub buyer_ssi_verified_by: Option<String>,
    pub ssi_status: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub expiry_date: Option<NaiveDate>,
    pub last_used_date: Option<NaiveDate>,
    pub usage_count: Option<i32>,

    // Compliance & Security
    pub sanctions_screened: Option<bool>,
    pub sanctions_screen_date: Option<DateTime<Utc>>,
    pub sanctions_screen_result: Option<String>,
    pub aml_check_passed: Option<bool>,
    pub aml_check_date: Option<DateTime<Utc>>,
    pub confidential: Option<bool>,
    pub access_restricted_to: Option<Vec<String>>,

    // Metadata & Audit
    pub settlement_currency: Option<String>,
    pub settlement_method: Option<String>,
    pub special_instructions: Option<String>,
    pub internal_notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new SSI record
#[instrument(skip(pool))]
pub async fn create_ssi_record(
    pool: &PgPool,
    req: CreateSsiRecordRequest,
) -> Result<SsiRecord, DbError> {
    let id = Uuid::new_v4();

    let record = sqlx::query_as!(
        SsiRecord,
        r#"
        INSERT INTO ssi_records (
            id, document_id, credit_agreement_id, deal_id, doc_hash, ssi_type,
            seller_ssi_id, seller_beneficiary_name, seller_bank_name, seller_bank_identifier,
            seller_swift_code, seller_aba_routing, seller_iban, seller_account_number,
            seller_reference_text, seller_bank_address, seller_intermediary_bank, seller_intermediary_swift,
            seller_custody_type, seller_custodian_name, seller_custodian_account_ref,
            seller_wallet_address, seller_wallet_chain, seller_wallet_chain_id, seller_wallet_type,
            buyer_ssi_id, buyer_beneficiary_name, buyer_bank_name, buyer_bank_identifier,
            buyer_swift_code, buyer_aba_routing, buyer_iban, buyer_account_number,
            buyer_reference_text, buyer_bank_address, buyer_intermediary_bank, buyer_intermediary_swift,
            buyer_custody_type, buyer_custodian_name, buyer_custodian_account_ref,
            buyer_wallet_address, buyer_wallet_chain, buyer_wallet_chain_id, buyer_wallet_type,
            seller_ssi_verified, seller_ssi_verified_at, seller_ssi_verified_by,
            buyer_ssi_verified, buyer_ssi_verified_at, buyer_ssi_verified_by,
            ssi_status, effective_date, expiry_date, last_used_date, usage_count,
            sanctions_screened, sanctions_screen_date, sanctions_screen_result,
            aml_check_passed, aml_check_date, confidential, access_restricted_to,
            settlement_currency, settlement_method, special_instructions, internal_notes,
            metadata, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6,
            $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
            $19, $20, $21, $22, $23, $24, $25,
            $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37,
            $38, $39, $40, $41, $42, $43, $44,
            $45, $46, $47, $48, $49, $50,
            $51, $52, $53, $54, $55, $56, $57, $58, $59,
            $60, $61, $62, $63, $64, $65, $66, $67, $68
        )
        RETURNING
            id, document_id, credit_agreement_id, deal_id, doc_hash, ssi_type,
            seller_ssi_id, seller_beneficiary_name, seller_bank_name, seller_bank_identifier,
            seller_swift_code, seller_aba_routing, seller_iban, seller_account_number,
            seller_reference_text, seller_bank_address, seller_intermediary_bank, seller_intermediary_swift,
            seller_custody_type, seller_custodian_name, seller_custodian_account_ref,
            seller_wallet_address, seller_wallet_chain, seller_wallet_chain_id, seller_wallet_type,
            buyer_ssi_id, buyer_beneficiary_name, buyer_bank_name, buyer_bank_identifier,
            buyer_swift_code, buyer_aba_routing, buyer_iban, buyer_account_number,
            buyer_reference_text, buyer_bank_address, buyer_intermediary_bank, buyer_intermediary_swift,
            buyer_custody_type, buyer_custodian_name, buyer_custodian_account_ref,
            buyer_wallet_address, buyer_wallet_chain, buyer_wallet_chain_id, buyer_wallet_type,
            seller_ssi_verified, seller_ssi_verified_at, seller_ssi_verified_by,
            buyer_ssi_verified, buyer_ssi_verified_at, buyer_ssi_verified_by,
            ssi_status, effective_date, expiry_date, last_used_date, usage_count,
            sanctions_screened, sanctions_screen_date, sanctions_screen_result,
            aml_check_passed, aml_check_date, confidential, access_restricted_to,
            settlement_currency, settlement_method, special_instructions, internal_notes,
            metadata,
            created_at, updated_at,
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.deal_id,
        req.doc_hash,
        req.ssi_type,
        req.seller_ssi_id,
        req.seller_beneficiary_name,
        req.seller_bank_name,
        req.seller_bank_identifier,
        req.seller_swift_code,
        req.seller_aba_routing,
        req.seller_iban,
        req.seller_account_number,
        req.seller_reference_text,
        req.seller_bank_address,
        req.seller_intermediary_bank,
        req.seller_intermediary_swift,
        req.seller_custody_type,
        req.seller_custodian_name,
        req.seller_custodian_account_ref,
        req.seller_wallet_address,
        req.seller_wallet_chain,
        req.seller_wallet_chain_id,
        req.seller_wallet_type,
        req.buyer_ssi_id,
        req.buyer_beneficiary_name,
        req.buyer_bank_name,
        req.buyer_bank_identifier,
        req.buyer_swift_code,
        req.buyer_aba_routing,
        req.buyer_iban,
        req.buyer_account_number,
        req.buyer_reference_text,
        req.buyer_bank_address,
        req.buyer_intermediary_bank,
        req.buyer_intermediary_swift,
        req.buyer_custody_type,
        req.buyer_custodian_name,
        req.buyer_custodian_account_ref,
        req.buyer_wallet_address,
        req.buyer_wallet_chain,
        req.buyer_wallet_chain_id,
        req.buyer_wallet_type,
        req.seller_ssi_verified,
        req.seller_ssi_verified_at,
        req.seller_ssi_verified_by,
        req.buyer_ssi_verified,
        req.buyer_ssi_verified_at,
        req.buyer_ssi_verified_by,
        req.ssi_status,
        req.effective_date,
        req.expiry_date,
        req.last_used_date,
        req.usage_count,
        req.sanctions_screened,
        req.sanctions_screen_date,
        req.sanctions_screen_result,
        req.aml_check_passed,
        req.aml_check_date,
        req.confidential,
        req.access_restricted_to.as_deref(),
        req.settlement_currency,
        req.settlement_method,
        req.special_instructions,
        req.internal_notes,
        req.metadata,
        req.created_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get SSI record by ID
#[instrument(skip(pool))]
pub async fn get_ssi_record_by_id(pool: &PgPool, id: Uuid) -> Result<Option<SsiRecord>, DbError> {
    let record = sqlx::query_as!(
        SsiRecord,
        r#"
        SELECT
            id, document_id, credit_agreement_id, deal_id, doc_hash, ssi_type,
            seller_ssi_id, seller_beneficiary_name, seller_bank_name, seller_bank_identifier,
            seller_swift_code, seller_aba_routing, seller_iban, seller_account_number,
            seller_reference_text, seller_bank_address, seller_intermediary_bank, seller_intermediary_swift,
            seller_custody_type, seller_custodian_name, seller_custodian_account_ref,
            seller_wallet_address, seller_wallet_chain, seller_wallet_chain_id, seller_wallet_type,
            buyer_ssi_id, buyer_beneficiary_name, buyer_bank_name, buyer_bank_identifier,
            buyer_swift_code, buyer_aba_routing, buyer_iban, buyer_account_number,
            buyer_reference_text, buyer_bank_address, buyer_intermediary_bank, buyer_intermediary_swift,
            buyer_custody_type, buyer_custodian_name, buyer_custodian_account_ref,
            buyer_wallet_address, buyer_wallet_chain, buyer_wallet_chain_id, buyer_wallet_type,
            seller_ssi_verified, seller_ssi_verified_at, seller_ssi_verified_by,
            buyer_ssi_verified, buyer_ssi_verified_at, buyer_ssi_verified_by,
            ssi_status, effective_date, expiry_date, last_used_date, usage_count,
            sanctions_screened, sanctions_screen_date, sanctions_screen_result,
            aml_check_passed, aml_check_date, confidential, access_restricted_to,
            settlement_currency, settlement_method, special_instructions, internal_notes,
            metadata,
            created_at, updated_at,
            created_by, updated_by
        FROM ssi_records WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get SSI record by document ID
#[instrument(skip(pool))]
pub async fn get_ssi_record_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<SsiRecord>, DbError> {
    let record = sqlx::query_as!(
        SsiRecord,
        r#"
        SELECT
            id, document_id, credit_agreement_id, deal_id, doc_hash, ssi_type,
            seller_ssi_id, seller_beneficiary_name, seller_bank_name, seller_bank_identifier,
            seller_swift_code, seller_aba_routing, seller_iban, seller_account_number,
            seller_reference_text, seller_bank_address, seller_intermediary_bank, seller_intermediary_swift,
            seller_custody_type, seller_custodian_name, seller_custodian_account_ref,
            seller_wallet_address, seller_wallet_chain, seller_wallet_chain_id, seller_wallet_type,
            buyer_ssi_id, buyer_beneficiary_name, buyer_bank_name, buyer_bank_identifier,
            buyer_swift_code, buyer_aba_routing, buyer_iban, buyer_account_number,
            buyer_reference_text, buyer_bank_address, buyer_intermediary_bank, buyer_intermediary_swift,
            buyer_custody_type, buyer_custodian_name, buyer_custodian_account_ref,
            buyer_wallet_address, buyer_wallet_chain, buyer_wallet_chain_id, buyer_wallet_type,
            seller_ssi_verified, seller_ssi_verified_at, seller_ssi_verified_by,
            buyer_ssi_verified, buyer_ssi_verified_at, buyer_ssi_verified_by,
            ssi_status, effective_date, expiry_date, last_used_date, usage_count,
            sanctions_screened, sanctions_screen_date, sanctions_screen_result,
            aml_check_passed, aml_check_date, confidential, access_restricted_to,
            settlement_currency, settlement_method, special_instructions, internal_notes,
            metadata,
            created_at, updated_at,
            created_by, updated_by
        FROM ssi_records WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List SSI records with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_ssi_records(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    ssi_status: Option<String>,
    ssi_type: Option<String>,
    settlement_method: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<SsiRecord>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_index = 3; // Start after limit and offset

    if credit_agreement_id.is_some() {
        where_clauses.push(format!("credit_agreement_id = ${}", param_index));
        param_index += 1;
    }
    if ssi_status.is_some() {
        where_clauses.push(format!("ssi_status = ${}", param_index));
        param_index += 1;
    }
    if ssi_type.is_some() {
        where_clauses.push(format!("ssi_type = ${}", param_index));
        param_index += 1;
    }
    if settlement_method.is_some() {
        where_clauses.push(format!("settlement_method = ${}", param_index));
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("ssi_status") => "ssi_status",
        Some("effective_date") => "effective_date",
        Some("expiry_date") => "expiry_date",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!("SELECT COUNT(*) as count FROM ssi_records {}", where_clause);

    let mut count_query_builder = sqlx::query_scalar(&count_query);
    if credit_agreement_id.is_some() {
        count_query_builder = count_query_builder.bind(&credit_agreement_id);
    }
    if ssi_status.is_some() {
        count_query_builder = count_query_builder.bind(&ssi_status);
    }
    if ssi_type.is_some() {
        count_query_builder = count_query_builder.bind(&ssi_type);
    }
    if settlement_method.is_some() {
        count_query_builder = count_query_builder.bind(&settlement_method);
    }

    let total: i64 = count_query_builder.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM ssi_records {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut query_builder = sqlx::query_as::<_, SsiRecord>(&query)
        .bind(limit)
        .bind(offset);

    if credit_agreement_id.is_some() {
        query_builder = query_builder.bind(&credit_agreement_id);
    }
    if ssi_status.is_some() {
        query_builder = query_builder.bind(&ssi_status);
    }
    if ssi_type.is_some() {
        query_builder = query_builder.bind(&ssi_type);
    }
    if settlement_method.is_some() {
        query_builder = query_builder.bind(&settlement_method);
    }

    let records = query_builder.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update SSI record
#[instrument(skip(pool))]
pub async fn update_ssi_record(
    pool: &PgPool,
    id: Uuid,
    req: UpdateSsiRecordRequest,
    updated_by: Option<Uuid>,
) -> Result<SsiRecord, DbError> {
    // Get existing record
    let existing = get_ssi_record_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("SSI record {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        SsiRecord,
        r#"
        UPDATE ssi_records SET
            document_id = $2,
            credit_agreement_id = $3,
            deal_id = $4,
            doc_hash = $5,
            ssi_type = $6,
            seller_ssi_id = $7,
            seller_beneficiary_name = $8,
            seller_bank_name = $9,
            seller_bank_identifier = $10,
            seller_swift_code = $11,
            seller_aba_routing = $12,
            seller_iban = $13,
            seller_account_number = $14,
            seller_reference_text = $15,
            seller_bank_address = $16,
            seller_intermediary_bank = $17,
            seller_intermediary_swift = $18,
            seller_custody_type = $19,
            seller_custodian_name = $20,
            seller_custodian_account_ref = $21,
            seller_wallet_address = $22,
            seller_wallet_chain = $23,
            seller_wallet_chain_id = $24,
            seller_wallet_type = $25,
            buyer_ssi_id = $26,
            buyer_beneficiary_name = $27,
            buyer_bank_name = $28,
            buyer_bank_identifier = $29,
            buyer_swift_code = $30,
            buyer_aba_routing = $31,
            buyer_iban = $32,
            buyer_account_number = $33,
            buyer_reference_text = $34,
            buyer_bank_address = $35,
            buyer_intermediary_bank = $36,
            buyer_intermediary_swift = $37,
            buyer_custody_type = $38,
            buyer_custodian_name = $39,
            buyer_custodian_account_ref = $40,
            buyer_wallet_address = $41,
            buyer_wallet_chain = $42,
            buyer_wallet_chain_id = $43,
            buyer_wallet_type = $44,
            seller_ssi_verified = $45,
            seller_ssi_verified_at = $46,
            seller_ssi_verified_by = $47,
            buyer_ssi_verified = $48,
            buyer_ssi_verified_at = $49,
            buyer_ssi_verified_by = $50,
            ssi_status = $51,
            effective_date = $52,
            expiry_date = $53,
            last_used_date = $54,
            usage_count = $55,
            sanctions_screened = $56,
            sanctions_screen_date = $57,
            sanctions_screen_result = $58,
            aml_check_passed = $59,
            aml_check_date = $60,
            confidential = $61,
            access_restricted_to = $62,
            settlement_currency = $63,
            settlement_method = $64,
            special_instructions = $65,
            internal_notes = $66,
            metadata = $67,
            updated_by = $68
        WHERE id = $1
        RETURNING
            id, document_id, credit_agreement_id, deal_id, doc_hash, ssi_type,
            seller_ssi_id, seller_beneficiary_name, seller_bank_name, seller_bank_identifier,
            seller_swift_code, seller_aba_routing, seller_iban, seller_account_number,
            seller_reference_text, seller_bank_address, seller_intermediary_bank, seller_intermediary_swift,
            seller_custody_type, seller_custodian_name, seller_custodian_account_ref,
            seller_wallet_address, seller_wallet_chain, seller_wallet_chain_id, seller_wallet_type,
            buyer_ssi_id, buyer_beneficiary_name, buyer_bank_name, buyer_bank_identifier,
            buyer_swift_code, buyer_aba_routing, buyer_iban, buyer_account_number,
            buyer_reference_text, buyer_bank_address, buyer_intermediary_bank, buyer_intermediary_swift,
            buyer_custody_type, buyer_custodian_name, buyer_custodian_account_ref,
            buyer_wallet_address, buyer_wallet_chain, buyer_wallet_chain_id, buyer_wallet_type,
            seller_ssi_verified, seller_ssi_verified_at, seller_ssi_verified_by,
            buyer_ssi_verified, buyer_ssi_verified_at, buyer_ssi_verified_by,
            ssi_status, effective_date, expiry_date, last_used_date, usage_count,
            sanctions_screened, sanctions_screen_date, sanctions_screen_result,
            aml_check_passed, aml_check_date, confidential, access_restricted_to,
            settlement_currency, settlement_method, special_instructions, internal_notes,
            metadata,
            created_at, updated_at,
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.deal_id.or(existing.deal_id),
        req.doc_hash.unwrap_or(existing.doc_hash),
        req.ssi_type.or(existing.ssi_type),
        req.seller_ssi_id.unwrap_or(existing.seller_ssi_id),
        req.seller_beneficiary_name.unwrap_or(existing.seller_beneficiary_name),
        req.seller_bank_name.or(existing.seller_bank_name),
        req.seller_bank_identifier.or(existing.seller_bank_identifier),
        req.seller_swift_code.or(existing.seller_swift_code),
        req.seller_aba_routing.or(existing.seller_aba_routing),
        req.seller_iban.or(existing.seller_iban),
        req.seller_account_number.or(existing.seller_account_number),
        req.seller_reference_text.or(existing.seller_reference_text),
        req.seller_bank_address.or(existing.seller_bank_address),
        req.seller_intermediary_bank.or(existing.seller_intermediary_bank),
        req.seller_intermediary_swift.or(existing.seller_intermediary_swift),
        req.seller_custody_type.or(existing.seller_custody_type),
        req.seller_custodian_name.or(existing.seller_custodian_name),
        req.seller_custodian_account_ref.or(existing.seller_custodian_account_ref),
        req.seller_wallet_address.or(existing.seller_wallet_address),
        req.seller_wallet_chain.or(existing.seller_wallet_chain),
        req.seller_wallet_chain_id.or(existing.seller_wallet_chain_id),
        req.seller_wallet_type.or(existing.seller_wallet_type),
        req.buyer_ssi_id.unwrap_or(existing.buyer_ssi_id),
        req.buyer_beneficiary_name.unwrap_or(existing.buyer_beneficiary_name),
        req.buyer_bank_name.or(existing.buyer_bank_name),
        req.buyer_bank_identifier.or(existing.buyer_bank_identifier),
        req.buyer_swift_code.or(existing.buyer_swift_code),
        req.buyer_aba_routing.or(existing.buyer_aba_routing),
        req.buyer_iban.or(existing.buyer_iban),
        req.buyer_account_number.or(existing.buyer_account_number),
        req.buyer_reference_text.or(existing.buyer_reference_text),
        req.buyer_bank_address.or(existing.buyer_bank_address),
        req.buyer_intermediary_bank.or(existing.buyer_intermediary_bank),
        req.buyer_intermediary_swift.or(existing.buyer_intermediary_swift),
        req.buyer_custody_type.or(existing.buyer_custody_type),
        req.buyer_custodian_name.or(existing.buyer_custodian_name),
        req.buyer_custodian_account_ref.or(existing.buyer_custodian_account_ref),
        req.buyer_wallet_address.or(existing.buyer_wallet_address),
        req.buyer_wallet_chain.or(existing.buyer_wallet_chain),
        req.buyer_wallet_chain_id.or(existing.buyer_wallet_chain_id),
        req.buyer_wallet_type.or(existing.buyer_wallet_type),
        req.seller_ssi_verified.or(existing.seller_ssi_verified),
        req.seller_ssi_verified_at.or(existing.seller_ssi_verified_at),
        req.seller_ssi_verified_by.or(existing.seller_ssi_verified_by),
        req.buyer_ssi_verified.or(existing.buyer_ssi_verified),
        req.buyer_ssi_verified_at.or(existing.buyer_ssi_verified_at),
        req.buyer_ssi_verified_by.or(existing.buyer_ssi_verified_by),
        req.ssi_status.or(existing.ssi_status),
        req.effective_date.unwrap_or(existing.effective_date),
        req.expiry_date.or(existing.expiry_date),
        req.last_used_date.or(existing.last_used_date),
        req.usage_count.or(existing.usage_count),
        req.sanctions_screened.or(existing.sanctions_screened),
        req.sanctions_screen_date.or(existing.sanctions_screen_date),
        req.sanctions_screen_result.or(existing.sanctions_screen_result),
        req.aml_check_passed.or(existing.aml_check_passed),
        req.aml_check_date.or(existing.aml_check_date),
        req.confidential.or(existing.confidential),
        req.access_restricted_to.as_deref().or(existing.access_restricted_to.as_deref()),
        req.settlement_currency.or(existing.settlement_currency),
        req.settlement_method.or(existing.settlement_method),
        req.special_instructions.or(existing.special_instructions),
        req.internal_notes.or(existing.internal_notes),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete SSI record
#[instrument(skip(pool))]
pub async fn delete_ssi_record(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM ssi_records WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple SSI records in a batch
#[instrument(skip(pool, requests))]
pub async fn create_ssi_records_batch(
    pool: &PgPool,
    requests: Vec<CreateSsiRecordRequest>,
) -> Result<Vec<Result<SsiRecord, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_ssi_record(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
