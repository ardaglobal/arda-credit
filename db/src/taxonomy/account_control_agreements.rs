//! Account Control Agreements Database Operations
//!
//! Provides CRUD operations for account control agreement documents (DACA/ACA/SACA).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Account Control Agreement record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccountControlAgreement {
    pub id: Uuid,
    pub document_id: String,
    pub security_agreement_id: Option<Uuid>,
    pub credit_agreement_id: Option<Uuid>,
    pub agreement_type: String,
    pub doc_hash: String,
    pub debtor_name: String,
    pub secured_party: String,
    pub bank_name: String,
    pub bank_identifier: Option<String>,
    pub accounts_covered: Option<serde_json::Value>, // DEFAULT '[]'
    pub total_accounts_count: Option<i32>,           // DEFAULT 0
    pub control_level: String,
    pub control_trigger_event: Option<String>,
    pub control_activated: Option<bool>, // DEFAULT FALSE
    pub activation_date: Option<NaiveDate>,
    pub activation_trigger: Option<String>,
    pub debtor_withdrawal_rights: Option<String>,
    pub sweeps_permitted: Option<bool>, // DEFAULT FALSE
    pub sweep_terms: Option<String>,
    pub setoff_rights_waived: Option<bool>, // DEFAULT FALSE
    pub notice_required_for_control: Option<bool>, // DEFAULT TRUE
    pub notice_period_days: Option<i32>,
    pub notice_delivery_method: Option<String>,
    pub ucc9_control_perfected: Option<bool>, // DEFAULT FALSE
    pub perfection_date: Option<NaiveDate>,
    pub control_method: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub automatic_termination: Option<bool>, // DEFAULT FALSE
    pub bank_acknowledgment_received: Option<bool>, // DEFAULT FALSE
    pub bank_compliance_confirmed: Option<bool>, // DEFAULT FALSE
    pub bank_fees: Option<Decimal>,
    pub prior_liens_exist: Option<bool>, // DEFAULT FALSE
    pub prior_liens_description: Option<String>,
    pub subordination_agreements: Option<String>,
    pub collateral_account: Option<bool>,        // DEFAULT FALSE
    pub lockbox_account: Option<bool>,           // DEFAULT FALSE
    pub collections_account: Option<bool>,       // DEFAULT FALSE
    pub linked_to_cash_management: Option<bool>, // DEFAULT FALSE
    pub cash_management_system: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>, // DEFAULT '{}'
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create an account control agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccountControlAgreementRequest {
    pub document_id: String,
    pub security_agreement_id: Option<Uuid>,
    pub credit_agreement_id: Option<Uuid>,
    pub agreement_type: String,
    pub doc_hash: String,
    pub debtor_name: String,
    pub secured_party: String,
    pub bank_name: String,
    pub bank_identifier: Option<String>,
    pub accounts_covered: Option<serde_json::Value>,
    pub total_accounts_count: Option<i32>,
    pub control_level: String,
    pub control_trigger_event: Option<String>,
    pub control_activated: Option<bool>,
    pub activation_date: Option<NaiveDate>,
    pub activation_trigger: Option<String>,
    pub debtor_withdrawal_rights: Option<String>,
    pub sweeps_permitted: Option<bool>,
    pub sweep_terms: Option<String>,
    pub setoff_rights_waived: Option<bool>,
    pub notice_required_for_control: Option<bool>,
    pub notice_period_days: Option<i32>,
    pub notice_delivery_method: Option<String>,
    pub ucc9_control_perfected: Option<bool>,
    pub perfection_date: Option<NaiveDate>,
    pub control_method: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub automatic_termination: Option<bool>,
    pub bank_acknowledgment_received: Option<bool>,
    pub bank_compliance_confirmed: Option<bool>,
    pub bank_fees: Option<Decimal>,
    pub prior_liens_exist: Option<bool>,
    pub prior_liens_description: Option<String>,
    pub subordination_agreements: Option<String>,
    pub collateral_account: Option<bool>,
    pub lockbox_account: Option<bool>,
    pub collections_account: Option<bool>,
    pub linked_to_cash_management: Option<bool>,
    pub cash_management_system: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update an account control agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAccountControlAgreementRequest {
    pub document_id: Option<String>,
    pub security_agreement_id: Option<Uuid>,
    pub credit_agreement_id: Option<Uuid>,
    pub agreement_type: Option<String>,
    pub doc_hash: Option<String>,
    pub debtor_name: Option<String>,
    pub secured_party: Option<String>,
    pub bank_name: Option<String>,
    pub bank_identifier: Option<String>,
    pub accounts_covered: Option<serde_json::Value>,
    pub total_accounts_count: Option<i32>,
    pub control_level: Option<String>,
    pub control_trigger_event: Option<String>,
    pub control_activated: Option<bool>,
    pub activation_date: Option<NaiveDate>,
    pub activation_trigger: Option<String>,
    pub debtor_withdrawal_rights: Option<String>,
    pub sweeps_permitted: Option<bool>,
    pub sweep_terms: Option<String>,
    pub setoff_rights_waived: Option<bool>,
    pub notice_required_for_control: Option<bool>,
    pub notice_period_days: Option<i32>,
    pub notice_delivery_method: Option<String>,
    pub ucc9_control_perfected: Option<bool>,
    pub perfection_date: Option<NaiveDate>,
    pub control_method: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub automatic_termination: Option<bool>,
    pub bank_acknowledgment_received: Option<bool>,
    pub bank_compliance_confirmed: Option<bool>,
    pub bank_fees: Option<Decimal>,
    pub prior_liens_exist: Option<bool>,
    pub prior_liens_description: Option<String>,
    pub subordination_agreements: Option<String>,
    pub collateral_account: Option<bool>,
    pub lockbox_account: Option<bool>,
    pub collections_account: Option<bool>,
    pub linked_to_cash_management: Option<bool>,
    pub cash_management_system: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new account control agreement
#[instrument(skip(pool))]
pub async fn create_account_control_agreement(
    pool: &PgPool,
    req: CreateAccountControlAgreementRequest,
) -> Result<AccountControlAgreement, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        AccountControlAgreement,
        r#"
        INSERT INTO account_control_agreements (
            id, document_id, security_agreement_id, credit_agreement_id, agreement_type,
            doc_hash, debtor_name, secured_party, bank_name, bank_identifier,
            accounts_covered, total_accounts_count, control_level, control_trigger_event,
            control_activated, activation_date, activation_trigger, debtor_withdrawal_rights,
            sweeps_permitted, sweep_terms, setoff_rights_waived, notice_required_for_control,
            notice_period_days, notice_delivery_method, ucc9_control_perfected, perfection_date,
            control_method, effective_date, termination_date, termination_conditions,
            automatic_termination, bank_acknowledgment_received, bank_compliance_confirmed,
            bank_fees, prior_liens_exist, prior_liens_description, subordination_agreements,
            collateral_account, lockbox_account, collections_account, linked_to_cash_management,
            cash_management_system, notes, metadata, created_at, updated_at, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
            $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34,
            $35, $36, $37, $38, $39, $40, $41, $42, $43, $44, $45, $46, $47
        )
        RETURNING
            id, document_id, security_agreement_id, credit_agreement_id, agreement_type,
            doc_hash, debtor_name, secured_party, bank_name, bank_identifier,
            accounts_covered, total_accounts_count, control_level, control_trigger_event,
            control_activated, activation_date, activation_trigger, debtor_withdrawal_rights,
            sweeps_permitted, sweep_terms, setoff_rights_waived, notice_required_for_control,
            notice_period_days, notice_delivery_method, ucc9_control_perfected, perfection_date,
            control_method, effective_date, termination_date, termination_conditions,
            automatic_termination, bank_acknowledgment_received, bank_compliance_confirmed,
            bank_fees, prior_liens_exist, prior_liens_description, subordination_agreements,
            collateral_account, lockbox_account, collections_account, linked_to_cash_management,
            cash_management_system, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.security_agreement_id,
        req.credit_agreement_id,
        req.agreement_type,
        req.doc_hash,
        req.debtor_name,
        req.secured_party,
        req.bank_name,
        req.bank_identifier,
        req.accounts_covered,
        req.total_accounts_count,
        req.control_level,
        req.control_trigger_event,
        req.control_activated, // nullable with DEFAULT
        req.activation_date,
        req.activation_trigger,
        req.debtor_withdrawal_rights,
        req.sweeps_permitted, // nullable with DEFAULT
        req.sweep_terms,
        req.setoff_rights_waived,        // nullable with DEFAULT
        req.notice_required_for_control, // nullable with DEFAULT
        req.notice_period_days,
        req.notice_delivery_method,
        req.ucc9_control_perfected, // nullable with DEFAULT
        req.perfection_date,
        req.control_method,
        req.effective_date,
        req.termination_date,
        req.termination_conditions,
        req.automatic_termination,        // nullable with DEFAULT
        req.bank_acknowledgment_received, // nullable with DEFAULT
        req.bank_compliance_confirmed,    // nullable with DEFAULT
        req.bank_fees,
        req.prior_liens_exist, // nullable with DEFAULT
        req.prior_liens_description,
        req.subordination_agreements,
        req.collateral_account,        // nullable with DEFAULT
        req.lockbox_account,           // nullable with DEFAULT
        req.collections_account,       // nullable with DEFAULT
        req.linked_to_cash_management, // nullable with DEFAULT
        req.cash_management_system,
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

/// Get account control agreement by ID
#[instrument(skip(pool))]
pub async fn get_account_control_agreement_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<AccountControlAgreement>, DbError> {
    let record = sqlx::query_as!(
        AccountControlAgreement,
        r#"
        SELECT
            id, document_id, security_agreement_id, credit_agreement_id, agreement_type,
            doc_hash, debtor_name, secured_party, bank_name, bank_identifier,
            accounts_covered, total_accounts_count, control_level, control_trigger_event,
            control_activated, activation_date, activation_trigger, debtor_withdrawal_rights,
            sweeps_permitted, sweep_terms, setoff_rights_waived, notice_required_for_control,
            notice_period_days, notice_delivery_method, ucc9_control_perfected, perfection_date,
            control_method, effective_date, termination_date, termination_conditions,
            automatic_termination, bank_acknowledgment_received, bank_compliance_confirmed,
            bank_fees, prior_liens_exist, prior_liens_description, subordination_agreements,
            collateral_account, lockbox_account, collections_account, linked_to_cash_management,
            cash_management_system, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM account_control_agreements WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get account control agreement by document ID
#[instrument(skip(pool))]
pub async fn get_account_control_agreement_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<AccountControlAgreement>, DbError> {
    let record = sqlx::query_as!(
        AccountControlAgreement,
        r#"
        SELECT
            id, document_id, security_agreement_id, credit_agreement_id, agreement_type,
            doc_hash, debtor_name, secured_party, bank_name, bank_identifier,
            accounts_covered, total_accounts_count, control_level, control_trigger_event,
            control_activated, activation_date, activation_trigger, debtor_withdrawal_rights,
            sweeps_permitted, sweep_terms, setoff_rights_waived, notice_required_for_control,
            notice_period_days, notice_delivery_method, ucc9_control_perfected, perfection_date,
            control_method, effective_date, termination_date, termination_conditions,
            automatic_termination, bank_acknowledgment_received, bank_compliance_confirmed,
            bank_fees, prior_liens_exist, prior_liens_description, subordination_agreements,
            collateral_account, lockbox_account, collections_account, linked_to_cash_management,
            cash_management_system, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM account_control_agreements WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List account control agreements with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_account_control_agreements(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    security_agreement_id: Option<Uuid>,
    agreement_type: Option<String>,
    control_level: Option<String>,
    control_activated: Option<bool>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<AccountControlAgreement>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_count = 3; // Start at 3 (limit, offset are 1, 2)

    if security_agreement_id.is_some() {
        param_count += 1;
        where_clauses.push(format!("security_agreement_id = ${}", param_count));
    }
    if agreement_type.is_some() {
        param_count += 1;
        where_clauses.push(format!("agreement_type = ${}", param_count));
    }
    if control_level.is_some() {
        param_count += 1;
        where_clauses.push(format!("control_level = ${}", param_count));
    }
    if control_activated.is_some() {
        param_count += 1;
        where_clauses.push(format!("control_activated = ${}", param_count));
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("agreement_type") => "agreement_type",
        Some("control_level") => "control_level",
        Some("effective_date") => "effective_date",
        Some("bank_name") => "bank_name",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM account_control_agreements {}",
        where_clause
    );

    let mut count_builder = sqlx::query_scalar(&count_query);
    if security_agreement_id.is_some() {
        count_builder = count_builder.bind(&security_agreement_id);
    }
    if let Some(ref at) = agreement_type {
        count_builder = count_builder.bind(at);
    }
    if let Some(ref cl) = control_level {
        count_builder = count_builder.bind(cl);
    }
    if control_activated.is_some() {
        count_builder = count_builder.bind(&control_activated);
    }

    let total: i64 = count_builder.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM account_control_agreements {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut query_builder = sqlx::query_as::<_, AccountControlAgreement>(&query)
        .bind(limit)
        .bind(offset);

    if security_agreement_id.is_some() {
        query_builder = query_builder.bind(&security_agreement_id);
    }
    if let Some(ref at) = agreement_type {
        query_builder = query_builder.bind(at);
    }
    if let Some(ref cl) = control_level {
        query_builder = query_builder.bind(cl);
    }
    if control_activated.is_some() {
        query_builder = query_builder.bind(&control_activated);
    }

    let records = query_builder.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update account control agreement
#[instrument(skip(pool))]
pub async fn update_account_control_agreement(
    pool: &PgPool,
    id: Uuid,
    req: UpdateAccountControlAgreementRequest,
    updated_by: Option<Uuid>,
) -> Result<AccountControlAgreement, DbError> {
    // Get existing record
    let existing = get_account_control_agreement_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Account control agreement {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        AccountControlAgreement,
        r#"
        UPDATE account_control_agreements SET
            document_id = $2,
            security_agreement_id = $3,
            credit_agreement_id = $4,
            agreement_type = $5,
            doc_hash = $6,
            debtor_name = $7,
            secured_party = $8,
            bank_name = $9,
            bank_identifier = $10,
            accounts_covered = $11,
            total_accounts_count = $12,
            control_level = $13,
            control_trigger_event = $14,
            control_activated = $15,
            activation_date = $16,
            activation_trigger = $17,
            debtor_withdrawal_rights = $18,
            sweeps_permitted = $19,
            sweep_terms = $20,
            setoff_rights_waived = $21,
            notice_required_for_control = $22,
            notice_period_days = $23,
            notice_delivery_method = $24,
            ucc9_control_perfected = $25,
            perfection_date = $26,
            control_method = $27,
            effective_date = $28,
            termination_date = $29,
            termination_conditions = $30,
            automatic_termination = $31,
            bank_acknowledgment_received = $32,
            bank_compliance_confirmed = $33,
            bank_fees = $34,
            prior_liens_exist = $35,
            prior_liens_description = $36,
            subordination_agreements = $37,
            collateral_account = $38,
            lockbox_account = $39,
            collections_account = $40,
            linked_to_cash_management = $41,
            cash_management_system = $42,
            notes = $43,
            metadata = $44,
            updated_by = $45
        WHERE id = $1
        RETURNING
            id, document_id, security_agreement_id, credit_agreement_id, agreement_type,
            doc_hash, debtor_name, secured_party, bank_name, bank_identifier,
            accounts_covered, total_accounts_count, control_level, control_trigger_event,
            control_activated, activation_date, activation_trigger, debtor_withdrawal_rights,
            sweeps_permitted, sweep_terms, setoff_rights_waived, notice_required_for_control,
            notice_period_days, notice_delivery_method, ucc9_control_perfected, perfection_date,
            control_method, effective_date, termination_date, termination_conditions,
            automatic_termination, bank_acknowledgment_received, bank_compliance_confirmed,
            bank_fees, prior_liens_exist, prior_liens_description, subordination_agreements,
            collateral_account, lockbox_account, collections_account, linked_to_cash_management,
            cash_management_system, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.security_agreement_id.or(existing.security_agreement_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.agreement_type.unwrap_or(existing.agreement_type),
        req.doc_hash.unwrap_or(existing.doc_hash),
        req.debtor_name.unwrap_or(existing.debtor_name),
        req.secured_party.unwrap_or(existing.secured_party),
        req.bank_name.unwrap_or(existing.bank_name),
        req.bank_identifier.or(existing.bank_identifier),
        req.accounts_covered.or(existing.accounts_covered),
        req.total_accounts_count.or(existing.total_accounts_count),
        req.control_level.unwrap_or(existing.control_level),
        req.control_trigger_event.or(existing.control_trigger_event),
        req.control_activated.or(existing.control_activated),
        req.activation_date.or(existing.activation_date),
        req.activation_trigger.or(existing.activation_trigger),
        req.debtor_withdrawal_rights
            .or(existing.debtor_withdrawal_rights),
        req.sweeps_permitted.or(existing.sweeps_permitted),
        req.sweep_terms.or(existing.sweep_terms),
        req.setoff_rights_waived.or(existing.setoff_rights_waived),
        req.notice_required_for_control
            .or(existing.notice_required_for_control),
        req.notice_period_days.or(existing.notice_period_days),
        req.notice_delivery_method
            .or(existing.notice_delivery_method),
        req.ucc9_control_perfected
            .or(existing.ucc9_control_perfected),
        req.perfection_date.or(existing.perfection_date),
        req.control_method.or(existing.control_method),
        req.effective_date.unwrap_or(existing.effective_date),
        req.termination_date.or(existing.termination_date),
        req.termination_conditions
            .or(existing.termination_conditions),
        req.automatic_termination.or(existing.automatic_termination),
        req.bank_acknowledgment_received
            .or(existing.bank_acknowledgment_received),
        req.bank_compliance_confirmed
            .or(existing.bank_compliance_confirmed),
        req.bank_fees.or(existing.bank_fees),
        req.prior_liens_exist.or(existing.prior_liens_exist),
        req.prior_liens_description
            .or(existing.prior_liens_description),
        req.subordination_agreements
            .or(existing.subordination_agreements),
        req.collateral_account.or(existing.collateral_account),
        req.lockbox_account.or(existing.lockbox_account),
        req.collections_account.or(existing.collections_account),
        req.linked_to_cash_management
            .or(existing.linked_to_cash_management),
        req.cash_management_system
            .or(existing.cash_management_system),
        req.notes.or(existing.notes),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete account control agreement
#[instrument(skip(pool))]
pub async fn delete_account_control_agreement(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM account_control_agreements WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple account control agreements in a batch
#[instrument(skip(pool, requests))]
pub async fn create_account_control_agreements_batch(
    pool: &PgPool,
    requests: Vec<CreateAccountControlAgreementRequest>,
) -> Result<Vec<Result<AccountControlAgreement, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_account_control_agreement(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
