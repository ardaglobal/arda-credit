//! Assignment & Assumption Agreements Database Operations
//!
//! Provides CRUD operations for assignment & assumption agreement documents.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Assignment & Assumption Agreement record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AssignmentAssumptionAgreement {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub doc_hash: String,
    pub assignment_type: String,
    pub assignor: String,
    pub assignor_entity_id: Option<Uuid>,
    pub assignee: String,
    pub assignee_entity_id: Option<Uuid>,
    pub administrative_agent: Option<String>,
    pub borrower_name: String,
    pub facility_assigned: Option<String>,
    pub assigned_commitment_amount: Option<Decimal>,
    pub assigned_outstanding_principal: Option<Decimal>,
    pub assigned_percentage: Option<Decimal>,
    pub pro_rata_share_before: Option<Decimal>,
    pub pro_rata_share_after_assignor: Option<Decimal>,
    pub pro_rata_share_assignee: Option<Decimal>,
    pub purchase_price: Option<Decimal>,
    pub price_per_unit: Option<Decimal>,
    pub accrued_interest: Option<Decimal>,
    pub fees_assigned: Option<Decimal>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_consent_date: Option<NaiveDate>,
    pub agent_consent_required: Option<bool>,
    pub agent_consent_obtained: Option<bool>,
    pub agent_consent_date: Option<NaiveDate>,
    pub other_consents_required: Option<String>,
    pub all_consents_obtained: Option<bool>,
    pub minimum_assignment_amount: Option<Decimal>,
    pub meets_minimum_threshold: Option<bool>,
    pub eligible_assignee: Option<bool>,
    pub eligibility_criteria_met: Option<String>,
    pub disqualified_lender_check: Option<bool>,
    pub not_on_dql: Option<bool>,
    pub assignor_representations: Option<serde_json::Value>,
    pub assignee_representations: Option<serde_json::Value>,
    pub processing_fee: Option<Decimal>,
    pub processing_fee_paid_by: Option<String>,
    pub recorded_in_register: Option<bool>,
    pub register_update_date: Option<NaiveDate>,
    pub register_notation: Option<String>,
    pub assignee_kyc_completed: Option<bool>,
    pub assignee_kyc_date: Option<NaiveDate>,
    pub assignee_tax_forms_received: Option<bool>,
    pub is_assignment: Option<bool>,
    pub changes_lender_of_record: Option<bool>,
    pub joinder_agreement_required: Option<bool>,
    pub joinder_executed: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub asset_transferred: Option<bool>,
    pub settlement_confirmation: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create an assignment & assumption agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAssignmentAssumptionAgreementRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub doc_hash: String,
    pub assignment_type: String,
    pub assignor: String,
    pub assignor_entity_id: Option<Uuid>,
    pub assignee: String,
    pub assignee_entity_id: Option<Uuid>,
    pub administrative_agent: Option<String>,
    pub borrower_name: String,
    pub facility_assigned: Option<String>,
    pub assigned_commitment_amount: Option<Decimal>,
    pub assigned_outstanding_principal: Option<Decimal>,
    pub assigned_percentage: Option<Decimal>,
    pub pro_rata_share_before: Option<Decimal>,
    pub pro_rata_share_after_assignor: Option<Decimal>,
    pub pro_rata_share_assignee: Option<Decimal>,
    pub purchase_price: Option<Decimal>,
    pub price_per_unit: Option<Decimal>,
    pub accrued_interest: Option<Decimal>,
    pub fees_assigned: Option<Decimal>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_consent_date: Option<NaiveDate>,
    pub agent_consent_required: Option<bool>,
    pub agent_consent_obtained: Option<bool>,
    pub agent_consent_date: Option<NaiveDate>,
    pub other_consents_required: Option<String>,
    pub all_consents_obtained: Option<bool>,
    pub minimum_assignment_amount: Option<Decimal>,
    pub meets_minimum_threshold: Option<bool>,
    pub eligible_assignee: Option<bool>,
    pub eligibility_criteria_met: Option<String>,
    pub disqualified_lender_check: Option<bool>,
    pub not_on_dql: Option<bool>,
    pub assignor_representations: Option<serde_json::Value>,
    pub assignee_representations: Option<serde_json::Value>,
    pub processing_fee: Option<Decimal>,
    pub processing_fee_paid_by: Option<String>,
    pub recorded_in_register: Option<bool>,
    pub register_update_date: Option<NaiveDate>,
    pub register_notation: Option<String>,
    pub assignee_kyc_completed: Option<bool>,
    pub assignee_kyc_date: Option<NaiveDate>,
    pub assignee_tax_forms_received: Option<bool>,
    pub is_assignment: Option<bool>,
    pub changes_lender_of_record: Option<bool>,
    pub joinder_agreement_required: Option<bool>,
    pub joinder_executed: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub asset_transferred: Option<bool>,
    pub settlement_confirmation: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update an assignment & assumption agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateAssignmentAssumptionAgreementRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub doc_hash: Option<String>,
    pub assignment_type: Option<String>,
    pub assignor: Option<String>,
    pub assignor_entity_id: Option<Uuid>,
    pub assignee: Option<String>,
    pub assignee_entity_id: Option<Uuid>,
    pub administrative_agent: Option<String>,
    pub borrower_name: Option<String>,
    pub facility_assigned: Option<String>,
    pub assigned_commitment_amount: Option<Decimal>,
    pub assigned_outstanding_principal: Option<Decimal>,
    pub assigned_percentage: Option<Decimal>,
    pub pro_rata_share_before: Option<Decimal>,
    pub pro_rata_share_after_assignor: Option<Decimal>,
    pub pro_rata_share_assignee: Option<Decimal>,
    pub purchase_price: Option<Decimal>,
    pub price_per_unit: Option<Decimal>,
    pub accrued_interest: Option<Decimal>,
    pub fees_assigned: Option<Decimal>,
    pub trade_date: Option<NaiveDate>,
    pub settlement_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_consent_date: Option<NaiveDate>,
    pub agent_consent_required: Option<bool>,
    pub agent_consent_obtained: Option<bool>,
    pub agent_consent_date: Option<NaiveDate>,
    pub other_consents_required: Option<String>,
    pub all_consents_obtained: Option<bool>,
    pub minimum_assignment_amount: Option<Decimal>,
    pub meets_minimum_threshold: Option<bool>,
    pub eligible_assignee: Option<bool>,
    pub eligibility_criteria_met: Option<String>,
    pub disqualified_lender_check: Option<bool>,
    pub not_on_dql: Option<bool>,
    pub assignor_representations: Option<serde_json::Value>,
    pub assignee_representations: Option<serde_json::Value>,
    pub processing_fee: Option<Decimal>,
    pub processing_fee_paid_by: Option<String>,
    pub recorded_in_register: Option<bool>,
    pub register_update_date: Option<NaiveDate>,
    pub register_notation: Option<String>,
    pub assignee_kyc_completed: Option<bool>,
    pub assignee_kyc_date: Option<NaiveDate>,
    pub assignee_tax_forms_received: Option<bool>,
    pub is_assignment: Option<bool>,
    pub changes_lender_of_record: Option<bool>,
    pub joinder_agreement_required: Option<bool>,
    pub joinder_executed: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub asset_transferred: Option<bool>,
    pub settlement_confirmation: Option<String>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new assignment & assumption agreement
#[instrument(skip(pool))]
pub async fn create_assignment_assumption_agreement(
    pool: &PgPool,
    req: CreateAssignmentAssumptionAgreementRequest,
) -> Result<AssignmentAssumptionAgreement, DbError> {
    let id = Uuid::new_v4();

    let record = sqlx::query_as!(
        AssignmentAssumptionAgreement,
        r#"
        INSERT INTO assignment_assumption_agreements (
            id, document_id, credit_agreement_id, trade_id, doc_hash, assignment_type,
            assignor, assignor_entity_id, assignee, assignee_entity_id, administrative_agent,
            borrower_name, facility_assigned, assigned_commitment_amount, assigned_outstanding_principal,
            assigned_percentage, pro_rata_share_before, pro_rata_share_after_assignor, pro_rata_share_assignee,
            purchase_price, price_per_unit, accrued_interest, fees_assigned, trade_date, settlement_date,
            effective_date, borrower_consent_required, borrower_consent_obtained, borrower_consent_date,
            agent_consent_required, agent_consent_obtained, agent_consent_date, other_consents_required,
            all_consents_obtained, minimum_assignment_amount, meets_minimum_threshold, eligible_assignee,
            eligibility_criteria_met, disqualified_lender_check, not_on_dql, assignor_representations,
            assignee_representations, processing_fee, processing_fee_paid_by, recorded_in_register,
            register_update_date, register_notation, assignee_kyc_completed, assignee_kyc_date,
            assignee_tax_forms_received, is_assignment, changes_lender_of_record, joinder_agreement_required,
            joinder_executed, agreement_status, cash_settled, asset_transferred, settlement_confirmation,
            notes, special_terms, metadata
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
            $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36, $37, $38,
            $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53, $54, $55, $56,
            $57, $58, $59, $60, $61
        )
        RETURNING *
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.trade_id,
        req.doc_hash,
        req.assignment_type,
        req.assignor,
        req.assignor_entity_id,
        req.assignee,
        req.assignee_entity_id,
        req.administrative_agent,
        req.borrower_name,
        req.facility_assigned,
        req.assigned_commitment_amount,
        req.assigned_outstanding_principal,
        req.assigned_percentage,
        req.pro_rata_share_before,
        req.pro_rata_share_after_assignor,
        req.pro_rata_share_assignee,
        req.purchase_price,
        req.price_per_unit,
        req.accrued_interest,
        req.fees_assigned,
        req.trade_date,
        req.settlement_date,
        req.effective_date,
        req.borrower_consent_required,
        req.borrower_consent_obtained,
        req.borrower_consent_date,
        req.agent_consent_required,
        req.agent_consent_obtained,
        req.agent_consent_date,
        req.other_consents_required,
        req.all_consents_obtained,
        req.minimum_assignment_amount,
        req.meets_minimum_threshold,
        req.eligible_assignee,
        req.eligibility_criteria_met,
        req.disqualified_lender_check,
        req.not_on_dql,
        req.assignor_representations,
        req.assignee_representations,
        req.processing_fee,
        req.processing_fee_paid_by,
        req.recorded_in_register,
        req.register_update_date,
        req.register_notation,
        req.assignee_kyc_completed,
        req.assignee_kyc_date,
        req.assignee_tax_forms_received,
        req.is_assignment,
        req.changes_lender_of_record,
        req.joinder_agreement_required,
        req.joinder_executed,
        req.agreement_status,
        req.cash_settled,
        req.asset_transferred,
        req.settlement_confirmation,
        req.notes,
        req.special_terms,
        req.metadata,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Get assignment & assumption agreement by ID
#[instrument(skip(pool))]
pub async fn get_assignment_assumption_agreement_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<AssignmentAssumptionAgreement>, DbError> {
    let record = sqlx::query_as!(
        AssignmentAssumptionAgreement,
        r#"
        SELECT * FROM assignment_assumption_agreements WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get assignment & assumption agreement by document ID
#[instrument(skip(pool))]
pub async fn get_assignment_assumption_agreement_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<AssignmentAssumptionAgreement>, DbError> {
    let record = sqlx::query_as!(
        AssignmentAssumptionAgreement,
        r#"
        SELECT * FROM assignment_assumption_agreements WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List assignment & assumption agreements with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_assignment_assumption_agreements(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    assignment_type: Option<String>,
    agreement_status: Option<String>,
    trade_date: Option<NaiveDate>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<AssignmentAssumptionAgreement>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut bind_index = 3;

    if credit_agreement_id.is_some() {
        where_clauses.push(format!("credit_agreement_id = ${}", bind_index));
        bind_index += 1;
    }
    if assignment_type.is_some() {
        where_clauses.push(format!("assignment_type = ${}", bind_index));
        bind_index += 1;
    }
    if agreement_status.is_some() {
        where_clauses.push(format!("agreement_status = ${}", bind_index));
        bind_index += 1;
    }
    if trade_date.is_some() {
        where_clauses.push(format!("trade_date = ${}", bind_index));
    }

    let where_clause = if where_clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", where_clauses.join(" AND "))
    };

    // Validate and set sort parameters
    let sort_column = match sort_by.as_deref() {
        Some("document_id") => "document_id",
        Some("trade_date") => "trade_date",
        Some("settlement_date") => "settlement_date",
        Some("effective_date") => "effective_date",
        Some("agreement_status") => "agreement_status",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM assignment_assumption_agreements {}",
        where_clause
    );

    let mut count_query = sqlx::query_scalar(&count_query).bind(limit).bind(offset);

    if let Some(ref ca_id) = credit_agreement_id {
        count_query = count_query.bind(ca_id);
    }
    if let Some(ref at) = assignment_type {
        count_query = count_query.bind(at);
    }
    if let Some(ref status) = agreement_status {
        count_query = count_query.bind(status);
    }
    if let Some(ref td) = trade_date {
        count_query = count_query.bind(td);
    }

    let total: i64 = count_query.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM assignment_assumption_agreements {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut records_query = sqlx::query_as::<_, AssignmentAssumptionAgreement>(&query)
        .bind(limit)
        .bind(offset);

    if let Some(ref ca_id) = credit_agreement_id {
        records_query = records_query.bind(ca_id);
    }
    if let Some(ref at) = assignment_type {
        records_query = records_query.bind(at);
    }
    if let Some(ref status) = agreement_status {
        records_query = records_query.bind(status);
    }
    if let Some(ref td) = trade_date {
        records_query = records_query.bind(td);
    }

    let records = records_query.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update assignment & assumption agreement
#[instrument(skip(pool))]
pub async fn update_assignment_assumption_agreement(
    pool: &PgPool,
    id: Uuid,
    req: UpdateAssignmentAssumptionAgreementRequest,
    updated_by: Option<Uuid>,
) -> Result<AssignmentAssumptionAgreement, DbError> {
    // Get existing record
    let existing = get_assignment_assumption_agreement_by_id(pool, id)
        .await?
        .ok_or_else(|| {
            DbError::NotFound(format!(
                "Assignment & assumption agreement {} not found",
                id
            ))
        })?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        AssignmentAssumptionAgreement,
        r#"
        UPDATE assignment_assumption_agreements SET
            document_id = $2,
            credit_agreement_id = $3,
            trade_id = $4,
            doc_hash = $5,
            assignment_type = $6,
            assignor = $7,
            assignor_entity_id = $8,
            assignee = $9,
            assignee_entity_id = $10,
            administrative_agent = $11,
            borrower_name = $12,
            facility_assigned = $13,
            assigned_commitment_amount = $14,
            assigned_outstanding_principal = $15,
            assigned_percentage = $16,
            pro_rata_share_before = $17,
            pro_rata_share_after_assignor = $18,
            pro_rata_share_assignee = $19,
            purchase_price = $20,
            price_per_unit = $21,
            accrued_interest = $22,
            fees_assigned = $23,
            trade_date = $24,
            settlement_date = $25,
            effective_date = $26,
            borrower_consent_required = $27,
            borrower_consent_obtained = $28,
            borrower_consent_date = $29,
            agent_consent_required = $30,
            agent_consent_obtained = $31,
            agent_consent_date = $32,
            other_consents_required = $33,
            all_consents_obtained = $34,
            minimum_assignment_amount = $35,
            meets_minimum_threshold = $36,
            eligible_assignee = $37,
            eligibility_criteria_met = $38,
            disqualified_lender_check = $39,
            not_on_dql = $40,
            assignor_representations = $41,
            assignee_representations = $42,
            processing_fee = $43,
            processing_fee_paid_by = $44,
            recorded_in_register = $45,
            register_update_date = $46,
            register_notation = $47,
            assignee_kyc_completed = $48,
            assignee_kyc_date = $49,
            assignee_tax_forms_received = $50,
            is_assignment = $51,
            changes_lender_of_record = $52,
            joinder_agreement_required = $53,
            joinder_executed = $54,
            agreement_status = $55,
            cash_settled = $56,
            asset_transferred = $57,
            settlement_confirmation = $58,
            notes = $59,
            special_terms = $60,
            metadata = $61,
            updated_by = $62
        WHERE id = $1
        RETURNING *
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.trade_id.or(existing.trade_id),
        req.doc_hash.unwrap_or(existing.doc_hash),
        req.assignment_type.unwrap_or(existing.assignment_type),
        req.assignor.unwrap_or(existing.assignor),
        req.assignor_entity_id.or(existing.assignor_entity_id),
        req.assignee.unwrap_or(existing.assignee),
        req.assignee_entity_id.or(existing.assignee_entity_id),
        req.administrative_agent.or(existing.administrative_agent),
        req.borrower_name.unwrap_or(existing.borrower_name),
        req.facility_assigned.or(existing.facility_assigned),
        req.assigned_commitment_amount
            .or(existing.assigned_commitment_amount),
        req.assigned_outstanding_principal
            .or(existing.assigned_outstanding_principal),
        req.assigned_percentage.or(existing.assigned_percentage),
        req.pro_rata_share_before.or(existing.pro_rata_share_before),
        req.pro_rata_share_after_assignor
            .or(existing.pro_rata_share_after_assignor),
        req.pro_rata_share_assignee
            .or(existing.pro_rata_share_assignee),
        req.purchase_price.or(existing.purchase_price),
        req.price_per_unit.or(existing.price_per_unit),
        req.accrued_interest.or(existing.accrued_interest),
        req.fees_assigned.or(existing.fees_assigned),
        req.trade_date.unwrap_or(existing.trade_date),
        req.settlement_date.unwrap_or(existing.settlement_date),
        req.effective_date.unwrap_or(existing.effective_date),
        req.borrower_consent_required
            .or(existing.borrower_consent_required),
        req.borrower_consent_obtained
            .or(existing.borrower_consent_obtained),
        req.borrower_consent_date.or(existing.borrower_consent_date),
        req.agent_consent_required
            .or(existing.agent_consent_required),
        req.agent_consent_obtained
            .or(existing.agent_consent_obtained),
        req.agent_consent_date.or(existing.agent_consent_date),
        req.other_consents_required
            .or(existing.other_consents_required),
        req.all_consents_obtained.or(existing.all_consents_obtained),
        req.minimum_assignment_amount
            .or(existing.minimum_assignment_amount),
        req.meets_minimum_threshold
            .or(existing.meets_minimum_threshold),
        req.eligible_assignee.or(existing.eligible_assignee),
        req.eligibility_criteria_met
            .or(existing.eligibility_criteria_met),
        req.disqualified_lender_check
            .or(existing.disqualified_lender_check),
        req.not_on_dql.or(existing.not_on_dql),
        req.assignor_representations
            .or(existing.assignor_representations),
        req.assignee_representations
            .or(existing.assignee_representations),
        req.processing_fee.or(existing.processing_fee),
        req.processing_fee_paid_by
            .or(existing.processing_fee_paid_by),
        req.recorded_in_register.or(existing.recorded_in_register),
        req.register_update_date.or(existing.register_update_date),
        req.register_notation.or(existing.register_notation),
        req.assignee_kyc_completed
            .or(existing.assignee_kyc_completed),
        req.assignee_kyc_date.or(existing.assignee_kyc_date),
        req.assignee_tax_forms_received
            .or(existing.assignee_tax_forms_received),
        req.is_assignment.or(existing.is_assignment),
        req.changes_lender_of_record
            .or(existing.changes_lender_of_record),
        req.joinder_agreement_required
            .or(existing.joinder_agreement_required),
        req.joinder_executed.or(existing.joinder_executed),
        req.agreement_status.or(existing.agreement_status),
        req.cash_settled.or(existing.cash_settled),
        req.asset_transferred.or(existing.asset_transferred),
        req.settlement_confirmation
            .or(existing.settlement_confirmation),
        req.notes.or(existing.notes),
        req.special_terms.or(existing.special_terms),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete assignment & assumption agreement
#[instrument(skip(pool))]
pub async fn delete_assignment_assumption_agreement(
    pool: &PgPool,
    id: Uuid,
) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM assignment_assumption_agreements WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple assignment & assumption agreements in a batch
#[instrument(skip(pool, requests))]
pub async fn create_assignment_assumption_agreements_batch(
    pool: &PgPool,
    requests: Vec<CreateAssignmentAssumptionAgreementRequest>,
) -> Result<Vec<Result<AssignmentAssumptionAgreement, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_assignment_assumption_agreement(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
