//! Security Agreements Database Operations
//!
//! Provides CRUD operations for security agreement documents.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Security Agreement record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SecurityAgreement {
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
    pub includes_accounts_receivable: Option<bool>,
    pub includes_inventory: Option<bool>,
    pub includes_equipment: Option<bool>,
    pub includes_intellectual_property: Option<bool>,
    pub includes_real_estate: Option<bool>,
    pub includes_deposit_accounts: Option<bool>,
    pub includes_investment_property: Option<bool>,
    pub includes_general_intangibles: Option<bool>,
    pub collateral_details: Option<serde_json::Value>,
    pub perfection_methods: Vec<String>,
    pub perfection_status: Option<String>,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<Uuid>,
    pub governing_law: String,
    pub filing_jurisdictions: Option<Vec<String>>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a security agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSecurityAgreementRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: String,
    pub grantor: String,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: String,
    pub collateral_agent: Option<String>,
    pub collateral_scope_summary: String,
    pub collateral_description: Option<String>,
    pub includes_accounts_receivable: Option<bool>,
    pub includes_inventory: Option<bool>,
    pub includes_equipment: Option<bool>,
    pub includes_intellectual_property: Option<bool>,
    pub includes_real_estate: Option<bool>,
    pub includes_deposit_accounts: Option<bool>,
    pub includes_investment_property: Option<bool>,
    pub includes_general_intangibles: Option<bool>,
    pub collateral_details: Option<serde_json::Value>,
    pub perfection_methods: Vec<String>,
    pub perfection_status: Option<String>,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<Uuid>,
    pub governing_law: String,
    pub filing_jurisdictions: Option<Vec<String>>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a security agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateSecurityAgreementRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub doc_hash: Option<String>,
    pub grantor: Option<String>,
    pub grantor_jurisdiction: Option<String>,
    pub secured_party: Option<String>,
    pub collateral_agent: Option<String>,
    pub collateral_scope_summary: Option<String>,
    pub collateral_description: Option<String>,
    pub includes_accounts_receivable: Option<bool>,
    pub includes_inventory: Option<bool>,
    pub includes_equipment: Option<bool>,
    pub includes_intellectual_property: Option<bool>,
    pub includes_real_estate: Option<bool>,
    pub includes_deposit_accounts: Option<bool>,
    pub includes_investment_property: Option<bool>,
    pub includes_general_intangibles: Option<bool>,
    pub collateral_details: Option<serde_json::Value>,
    pub perfection_methods: Option<Vec<String>>,
    pub perfection_status: Option<String>,
    pub perfection_date: Option<NaiveDate>,
    pub lien_priority: Option<String>,
    pub intercreditor_agreement_id: Option<Uuid>,
    pub governing_law: Option<String>,
    pub filing_jurisdictions: Option<Vec<String>>,
    pub excluded_collateral: Option<String>,
    pub permitted_liens: Option<String>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub release_conditions: Option<String>,
    pub partial_release_permitted: Option<bool>,
    pub maintenance_covenants: Option<String>,
    pub insurance_requirements: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new security agreement
#[instrument(skip(pool))]
pub async fn create_security_agreement(
    pool: &PgPool,
    req: CreateSecurityAgreementRequest,
) -> Result<SecurityAgreement, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        SecurityAgreement,
        r#"
        INSERT INTO security_agreements (
            id, document_id, credit_agreement_id, doc_hash, grantor, grantor_jurisdiction,
            secured_party, collateral_agent, collateral_scope_summary, collateral_description,
            includes_accounts_receivable, includes_inventory, includes_equipment,
            includes_intellectual_property, includes_real_estate, includes_deposit_accounts,
            includes_investment_property, includes_general_intangibles, collateral_details,
            perfection_methods, perfection_status, perfection_date, lien_priority,
            intercreditor_agreement_id, governing_law, filing_jurisdictions,
            excluded_collateral, permitted_liens, effective_date, termination_date,
            release_conditions, partial_release_permitted, maintenance_covenants,
            insurance_requirements, notes, metadata, created_at, updated_at, created_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
            $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
            $31, $32, $33, $34, $35, $36, $37, $38, $39
        )
        RETURNING
            id, document_id, credit_agreement_id, doc_hash, grantor, grantor_jurisdiction,
            secured_party, collateral_agent, collateral_scope_summary, collateral_description,
            includes_accounts_receivable, includes_inventory, includes_equipment,
            includes_intellectual_property, includes_real_estate, includes_deposit_accounts,
            includes_investment_property, includes_general_intangibles, collateral_details,
            perfection_methods, perfection_status, perfection_date, lien_priority,
            intercreditor_agreement_id, governing_law, filing_jurisdictions,
            excluded_collateral, permitted_liens, effective_date, termination_date,
            release_conditions, partial_release_permitted, maintenance_covenants,
            insurance_requirements, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.doc_hash,
        req.grantor,
        req.grantor_jurisdiction,
        req.secured_party,
        req.collateral_agent,
        req.collateral_scope_summary,
        req.collateral_description,
        req.includes_accounts_receivable.unwrap_or(false),
        req.includes_inventory.unwrap_or(false),
        req.includes_equipment.unwrap_or(false),
        req.includes_intellectual_property.unwrap_or(false),
        req.includes_real_estate.unwrap_or(false),
        req.includes_deposit_accounts.unwrap_or(false),
        req.includes_investment_property.unwrap_or(false),
        req.includes_general_intangibles.unwrap_or(false),
        req.collateral_details.unwrap_or(serde_json::json!({})),
        &req.perfection_methods,
        req.perfection_status
            .unwrap_or_else(|| "pending".to_string()),
        req.perfection_date,
        req.lien_priority,
        req.intercreditor_agreement_id,
        req.governing_law,
        req.filing_jurisdictions.as_deref(),
        req.excluded_collateral,
        req.permitted_liens,
        req.effective_date,
        req.termination_date,
        req.release_conditions,
        req.partial_release_permitted.unwrap_or(false),
        req.maintenance_covenants,
        req.insurance_requirements,
        req.notes,
        req.metadata.unwrap_or(serde_json::json!({})),
        now,
        now,
        req.created_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get security agreement by ID
#[instrument(skip(pool))]
pub async fn get_security_agreement_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<SecurityAgreement>, DbError> {
    let record = sqlx::query_as!(
        SecurityAgreement,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash, grantor, grantor_jurisdiction,
            secured_party, collateral_agent, collateral_scope_summary, collateral_description,
            includes_accounts_receivable, includes_inventory, includes_equipment,
            includes_intellectual_property, includes_real_estate, includes_deposit_accounts,
            includes_investment_property, includes_general_intangibles, collateral_details,
            perfection_methods, perfection_status, perfection_date, lien_priority,
            intercreditor_agreement_id, governing_law, filing_jurisdictions,
            excluded_collateral, permitted_liens, effective_date, termination_date,
            release_conditions, partial_release_permitted, maintenance_covenants,
            insurance_requirements, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM security_agreements WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get security agreement by document ID
#[instrument(skip(pool))]
pub async fn get_security_agreement_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<SecurityAgreement>, DbError> {
    let record = sqlx::query_as!(
        SecurityAgreement,
        r#"
        SELECT
            id, document_id, credit_agreement_id, doc_hash, grantor, grantor_jurisdiction,
            secured_party, collateral_agent, collateral_scope_summary, collateral_description,
            includes_accounts_receivable, includes_inventory, includes_equipment,
            includes_intellectual_property, includes_real_estate, includes_deposit_accounts,
            includes_investment_property, includes_general_intangibles, collateral_details,
            perfection_methods, perfection_status, perfection_date, lien_priority,
            intercreditor_agreement_id, governing_law, filing_jurisdictions,
            excluded_collateral, permitted_liens, effective_date, termination_date,
            release_conditions, partial_release_permitted, maintenance_covenants,
            insurance_requirements, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM security_agreements WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List security agreements with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_security_agreements(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    perfection_status: Option<String>,
    lien_priority: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<SecurityAgreement>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_index = 3; // Start at 3 (after limit and offset)

    if credit_agreement_id.is_some() {
        where_clauses.push(format!("credit_agreement_id = ${}", param_index));
        param_index += 1;
    }
    if perfection_status.is_some() {
        where_clauses.push(format!("perfection_status = ${}", param_index));
        param_index += 1;
    }
    if lien_priority.is_some() {
        where_clauses.push(format!("lien_priority = ${}", param_index));
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
        Some("grantor") => "grantor",
        Some("perfection_status") => "perfection_status",
        Some("lien_priority") => "lien_priority",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM security_agreements {}",
        where_clause
    );

    let total: i64 = sqlx::query_scalar(&count_query)
        .bind(&credit_agreement_id)
        .bind(&perfection_status)
        .bind(&lien_priority)
        .fetch_one(pool)
        .await?;

    // Get records
    let query = format!(
        "SELECT * FROM security_agreements {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let records = sqlx::query_as::<_, SecurityAgreement>(&query)
        .bind(limit)
        .bind(offset)
        .bind(&credit_agreement_id)
        .bind(&perfection_status)
        .bind(&lien_priority)
        .fetch_all(pool)
        .await?;

    Ok((records, total))
}

/// Update security agreement
#[instrument(skip(pool))]
pub async fn update_security_agreement(
    pool: &PgPool,
    id: Uuid,
    req: UpdateSecurityAgreementRequest,
    updated_by: Option<Uuid>,
) -> Result<SecurityAgreement, DbError> {
    // Apply updates using COALESCE to keep existing values when new value is NULL
    let record = sqlx::query_as!(
        SecurityAgreement,
        r#"
        UPDATE security_agreements SET
            document_id = COALESCE($2, document_id),
            credit_agreement_id = COALESCE($3, credit_agreement_id),
            doc_hash = COALESCE($4, doc_hash),
            grantor = COALESCE($5, grantor),
            grantor_jurisdiction = COALESCE($6, grantor_jurisdiction),
            secured_party = COALESCE($7, secured_party),
            collateral_agent = COALESCE($8, collateral_agent),
            collateral_scope_summary = COALESCE($9, collateral_scope_summary),
            collateral_description = COALESCE($10, collateral_description),
            includes_accounts_receivable = COALESCE($11, includes_accounts_receivable),
            includes_inventory = COALESCE($12, includes_inventory),
            includes_equipment = COALESCE($13, includes_equipment),
            includes_intellectual_property = COALESCE($14, includes_intellectual_property),
            includes_real_estate = COALESCE($15, includes_real_estate),
            includes_deposit_accounts = COALESCE($16, includes_deposit_accounts),
            includes_investment_property = COALESCE($17, includes_investment_property),
            includes_general_intangibles = COALESCE($18, includes_general_intangibles),
            collateral_details = COALESCE($19, collateral_details),
            perfection_methods = COALESCE($20, perfection_methods),
            perfection_status = COALESCE($21, perfection_status),
            perfection_date = COALESCE($22, perfection_date),
            lien_priority = COALESCE($23, lien_priority),
            intercreditor_agreement_id = COALESCE($24, intercreditor_agreement_id),
            governing_law = COALESCE($25, governing_law),
            filing_jurisdictions = COALESCE($26, filing_jurisdictions),
            excluded_collateral = COALESCE($27, excluded_collateral),
            permitted_liens = COALESCE($28, permitted_liens),
            effective_date = COALESCE($29, effective_date),
            termination_date = COALESCE($30, termination_date),
            release_conditions = COALESCE($31, release_conditions),
            partial_release_permitted = COALESCE($32, partial_release_permitted),
            maintenance_covenants = COALESCE($33, maintenance_covenants),
            insurance_requirements = COALESCE($34, insurance_requirements),
            notes = COALESCE($35, notes),
            metadata = COALESCE($36, metadata),
            updated_by = $37
        WHERE id = $1
        RETURNING
            id, document_id, credit_agreement_id, doc_hash, grantor, grantor_jurisdiction,
            secured_party, collateral_agent, collateral_scope_summary, collateral_description,
            includes_accounts_receivable, includes_inventory, includes_equipment,
            includes_intellectual_property, includes_real_estate, includes_deposit_accounts,
            includes_investment_property, includes_general_intangibles, collateral_details,
            perfection_methods, perfection_status, perfection_date, lien_priority,
            intercreditor_agreement_id, governing_law, filing_jurisdictions,
            excluded_collateral, permitted_liens, effective_date, termination_date,
            release_conditions, partial_release_permitted, maintenance_covenants,
            insurance_requirements, notes, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.doc_hash,
        req.grantor,
        req.grantor_jurisdiction,
        req.secured_party,
        req.collateral_agent,
        req.collateral_scope_summary,
        req.collateral_description,
        req.includes_accounts_receivable,
        req.includes_inventory,
        req.includes_equipment,
        req.includes_intellectual_property,
        req.includes_real_estate,
        req.includes_deposit_accounts,
        req.includes_investment_property,
        req.includes_general_intangibles,
        req.collateral_details,
        req.perfection_methods.as_deref(),
        req.perfection_status,
        req.perfection_date,
        req.lien_priority,
        req.intercreditor_agreement_id,
        req.governing_law,
        req.filing_jurisdictions.as_deref(),
        req.excluded_collateral,
        req.permitted_liens,
        req.effective_date,
        req.termination_date,
        req.release_conditions,
        req.partial_release_permitted,
        req.maintenance_covenants,
        req.insurance_requirements,
        req.notes,
        req.metadata,
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete security agreement
#[instrument(skip(pool))]
pub async fn delete_security_agreement(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM security_agreements WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple security agreements in a batch
#[instrument(skip(pool, requests))]
pub async fn create_security_agreements_batch(
    pool: &PgPool,
    requests: Vec<CreateSecurityAgreementRequest>,
) -> Result<Vec<Result<SecurityAgreement, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_security_agreement(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
