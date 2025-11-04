//! Participation Agreements Database Operations
//!
//! Provides CRUD operations for participation agreement documents.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::error::DbError;

/// Participation Agreement record with full document attributes
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ParticipationAgreement {
    pub id: Uuid,
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub doc_hash: String,
    pub participation_type: String,
    pub grantor: String,
    pub grantor_entity_id: Option<Uuid>,
    pub grantor_remains_lender_of_record: Option<bool>, // DEFAULT TRUE
    pub participant: String,
    pub participant_entity_id: Option<Uuid>,
    pub borrower_name: String,
    pub facility_reference: Option<String>,
    pub participation_percentage: Option<Decimal>,
    pub participation_amount: Option<Decimal>,
    pub grantor_total_commitment: Option<Decimal>,
    pub grantor_outstanding_principal: Option<Decimal>,
    pub purchase_price: Option<Decimal>,
    pub purchase_price_percentage: Option<Decimal>,
    pub funded_at_closing: Option<bool>, // DEFAULT TRUE
    pub funding_mechanism: Option<String>,
    pub payments_flow: String,
    pub payment_terms: Option<String>,
    pub participant_rights: Option<serde_json::Value>, // DEFAULT '{}'
    pub no_privity_with_borrower: Option<bool>,        // DEFAULT TRUE
    pub participant_has_voting_rights: Option<bool>,   // DEFAULT FALSE
    pub borrower_consent_required: Option<bool>,       // DEFAULT FALSE
    pub borrower_consent_obtained: Option<bool>,       // DEFAULT FALSE
    pub borrower_knowledge_of_participation: Option<bool>, // DEFAULT FALSE
    pub agent_notification_required: Option<bool>,     // DEFAULT FALSE
    pub agent_notified: Option<bool>,                  // DEFAULT FALSE
    pub pro_rata_sharing: Option<bool>,                // DEFAULT TRUE
    pub principal_sharing_percentage: Option<Decimal>,
    pub interest_sharing_percentage: Option<Decimal>,
    pub fee_sharing_percentage: Option<Decimal>,
    pub grantor_pays_participant: Option<bool>, // DEFAULT TRUE
    pub settlement_frequency: Option<String>,
    pub participant_credit_risk: Option<bool>, // DEFAULT TRUE
    pub participant_funding_risk: Option<bool>, // DEFAULT FALSE
    pub setoff_rights: Option<String>,
    pub netting_provisions: Option<String>,
    pub grantor_representations: Option<serde_json::Value>, // DEFAULT '{}'
    pub participant_representations: Option<serde_json::Value>, // DEFAULT '{}'
    pub participant_assignment_rights: Option<String>,
    pub requires_grantor_consent_to_assign: Option<bool>, // DEFAULT TRUE
    pub is_sub_participation: Option<bool>,               // DEFAULT FALSE
    pub parent_participation_id: Option<Uuid>,
    pub confidential_agreement: Option<bool>, // DEFAULT TRUE
    pub disclosure_restrictions: Option<String>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub participant_kyc_completed: Option<bool>, // DEFAULT FALSE
    pub participant_kyc_date: Option<NaiveDate>,
    pub participant_eligible: Option<bool>, // DEFAULT TRUE
    pub agreement_status: Option<String>,   // DEFAULT 'draft'
    pub cash_settled: Option<bool>,         // DEFAULT FALSE
    pub participation_active: Option<bool>, // DEFAULT FALSE
    pub total_payments_received: Option<Decimal>, // DEFAULT 0
    pub total_payments_to_participant: Option<Decimal>, // DEFAULT 0
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>, // DEFAULT '{}'
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
}

/// Request to create a participation agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateParticipationAgreementRequest {
    pub document_id: String,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub doc_hash: String,
    pub participation_type: String,
    pub grantor: String,
    pub grantor_entity_id: Option<Uuid>,
    pub grantor_remains_lender_of_record: Option<bool>,
    pub participant: String,
    pub participant_entity_id: Option<Uuid>,
    pub borrower_name: String,
    pub facility_reference: Option<String>,
    pub participation_percentage: Option<Decimal>,
    pub participation_amount: Option<Decimal>,
    pub grantor_total_commitment: Option<Decimal>,
    pub grantor_outstanding_principal: Option<Decimal>,
    pub purchase_price: Option<Decimal>,
    pub purchase_price_percentage: Option<Decimal>,
    pub funded_at_closing: Option<bool>,
    pub funding_mechanism: Option<String>,
    pub payments_flow: String,
    pub payment_terms: Option<String>,
    pub participant_rights: Option<serde_json::Value>,
    pub no_privity_with_borrower: Option<bool>,
    pub participant_has_voting_rights: Option<bool>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_knowledge_of_participation: Option<bool>,
    pub agent_notification_required: Option<bool>,
    pub agent_notified: Option<bool>,
    pub pro_rata_sharing: Option<bool>,
    pub principal_sharing_percentage: Option<Decimal>,
    pub interest_sharing_percentage: Option<Decimal>,
    pub fee_sharing_percentage: Option<Decimal>,
    pub grantor_pays_participant: Option<bool>,
    pub settlement_frequency: Option<String>,
    pub participant_credit_risk: Option<bool>,
    pub participant_funding_risk: Option<bool>,
    pub setoff_rights: Option<String>,
    pub netting_provisions: Option<String>,
    pub grantor_representations: Option<serde_json::Value>,
    pub participant_representations: Option<serde_json::Value>,
    pub participant_assignment_rights: Option<String>,
    pub requires_grantor_consent_to_assign: Option<bool>,
    pub is_sub_participation: Option<bool>,
    pub parent_participation_id: Option<Uuid>,
    pub confidential_agreement: Option<bool>,
    pub disclosure_restrictions: Option<String>,
    pub trade_date: NaiveDate,
    pub settlement_date: NaiveDate,
    pub effective_date: NaiveDate,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub participant_kyc_completed: Option<bool>,
    pub participant_kyc_date: Option<NaiveDate>,
    pub participant_eligible: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub participation_active: Option<bool>,
    pub total_payments_received: Option<Decimal>,
    pub total_payments_to_participant: Option<Decimal>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_by: Option<Uuid>,
}

/// Request to update a participation agreement
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateParticipationAgreementRequest {
    pub document_id: Option<String>,
    pub credit_agreement_id: Option<Uuid>,
    pub trade_id: Option<Uuid>,
    pub doc_hash: Option<String>,
    pub participation_type: Option<String>,
    pub grantor: Option<String>,
    pub grantor_entity_id: Option<Uuid>,
    pub grantor_remains_lender_of_record: Option<bool>,
    pub participant: Option<String>,
    pub participant_entity_id: Option<Uuid>,
    pub borrower_name: Option<String>,
    pub facility_reference: Option<String>,
    pub participation_percentage: Option<Decimal>,
    pub participation_amount: Option<Decimal>,
    pub grantor_total_commitment: Option<Decimal>,
    pub grantor_outstanding_principal: Option<Decimal>,
    pub purchase_price: Option<Decimal>,
    pub purchase_price_percentage: Option<Decimal>,
    pub funded_at_closing: Option<bool>,
    pub funding_mechanism: Option<String>,
    pub payments_flow: Option<String>,
    pub payment_terms: Option<String>,
    pub participant_rights: Option<serde_json::Value>,
    pub no_privity_with_borrower: Option<bool>,
    pub participant_has_voting_rights: Option<bool>,
    pub borrower_consent_required: Option<bool>,
    pub borrower_consent_obtained: Option<bool>,
    pub borrower_knowledge_of_participation: Option<bool>,
    pub agent_notification_required: Option<bool>,
    pub agent_notified: Option<bool>,
    pub pro_rata_sharing: Option<bool>,
    pub principal_sharing_percentage: Option<Decimal>,
    pub interest_sharing_percentage: Option<Decimal>,
    pub fee_sharing_percentage: Option<Decimal>,
    pub grantor_pays_participant: Option<bool>,
    pub settlement_frequency: Option<String>,
    pub participant_credit_risk: Option<bool>,
    pub participant_funding_risk: Option<bool>,
    pub setoff_rights: Option<String>,
    pub netting_provisions: Option<String>,
    pub grantor_representations: Option<serde_json::Value>,
    pub participant_representations: Option<serde_json::Value>,
    pub participant_assignment_rights: Option<String>,
    pub requires_grantor_consent_to_assign: Option<bool>,
    pub is_sub_participation: Option<bool>,
    pub parent_participation_id: Option<Uuid>,
    pub confidential_agreement: Option<bool>,
    pub disclosure_restrictions: Option<String>,
    pub trade_date: Option<NaiveDate>,
    pub settlement_date: Option<NaiveDate>,
    pub effective_date: Option<NaiveDate>,
    pub termination_date: Option<NaiveDate>,
    pub termination_conditions: Option<String>,
    pub participant_kyc_completed: Option<bool>,
    pub participant_kyc_date: Option<NaiveDate>,
    pub participant_eligible: Option<bool>,
    pub agreement_status: Option<String>,
    pub cash_settled: Option<bool>,
    pub participation_active: Option<bool>,
    pub total_payments_received: Option<Decimal>,
    pub total_payments_to_participant: Option<Decimal>,
    pub notes: Option<String>,
    pub special_terms: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub updated_by: Option<Uuid>,
}

/// Create a new participation agreement
#[instrument(skip(pool))]
pub async fn create_participation_agreement(
    pool: &PgPool,
    req: CreateParticipationAgreementRequest,
) -> Result<ParticipationAgreement, DbError> {
    let id = Uuid::new_v4();
    let now = Utc::now();

    let record = sqlx::query_as!(
        ParticipationAgreement,
        r#"
        INSERT INTO participation_agreements (
            id, document_id, credit_agreement_id, trade_id, doc_hash, participation_type,
            grantor, grantor_entity_id, grantor_remains_lender_of_record, participant,
            participant_entity_id, borrower_name, facility_reference, participation_percentage,
            participation_amount, grantor_total_commitment, grantor_outstanding_principal,
            purchase_price, purchase_price_percentage, funded_at_closing, funding_mechanism,
            payments_flow, payment_terms, participant_rights, no_privity_with_borrower,
            participant_has_voting_rights, borrower_consent_required, borrower_consent_obtained,
            borrower_knowledge_of_participation, agent_notification_required, agent_notified,
            pro_rata_sharing, principal_sharing_percentage, interest_sharing_percentage,
            fee_sharing_percentage, grantor_pays_participant, settlement_frequency,
            participant_credit_risk, participant_funding_risk, setoff_rights, netting_provisions,
            grantor_representations, participant_representations, participant_assignment_rights,
            requires_grantor_consent_to_assign, is_sub_participation, parent_participation_id,
            confidential_agreement, disclosure_restrictions, trade_date, settlement_date,
            effective_date, termination_date, termination_conditions, participant_kyc_completed,
            participant_kyc_date, participant_eligible, agreement_status, cash_settled,
            participation_active, total_payments_received, total_payments_to_participant,
            notes, special_terms, metadata, created_at, updated_at, created_by, updated_by
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19,
            $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30, $31, $32, $33, $34, $35, $36,
            $37, $38, $39, $40, $41, $42, $43, $44, $45, $46, $47, $48, $49, $50, $51, $52, $53,
            $54, $55, $56, $57, $58, $59, $60, $61, $62, $63, $64, $65, $66, $67, $68, $69
        )
        RETURNING
            id, document_id, credit_agreement_id, trade_id, doc_hash, participation_type,
            grantor, grantor_entity_id, grantor_remains_lender_of_record, participant,
            participant_entity_id, borrower_name, facility_reference, participation_percentage,
            participation_amount, grantor_total_commitment, grantor_outstanding_principal,
            purchase_price, purchase_price_percentage, funded_at_closing, funding_mechanism,
            payments_flow, payment_terms, participant_rights, no_privity_with_borrower,
            participant_has_voting_rights, borrower_consent_required, borrower_consent_obtained,
            borrower_knowledge_of_participation, agent_notification_required, agent_notified,
            pro_rata_sharing, principal_sharing_percentage, interest_sharing_percentage,
            fee_sharing_percentage, grantor_pays_participant, settlement_frequency,
            participant_credit_risk, participant_funding_risk, setoff_rights, netting_provisions,
            grantor_representations, participant_representations, participant_assignment_rights,
            requires_grantor_consent_to_assign, is_sub_participation, parent_participation_id,
            confidential_agreement, disclosure_restrictions, trade_date, settlement_date,
            effective_date, termination_date, termination_conditions, participant_kyc_completed,
            participant_kyc_date, participant_eligible, agreement_status, cash_settled,
            participation_active, total_payments_received, total_payments_to_participant,
            notes, special_terms, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id,
        req.credit_agreement_id,
        req.trade_id,
        req.doc_hash,
        req.participation_type,
        req.grantor,
        req.grantor_entity_id,
        req.grantor_remains_lender_of_record,
        req.participant,
        req.participant_entity_id,
        req.borrower_name,
        req.facility_reference,
        req.participation_percentage,
        req.participation_amount,
        req.grantor_total_commitment,
        req.grantor_outstanding_principal,
        req.purchase_price,
        req.purchase_price_percentage,
        req.funded_at_closing,
        req.funding_mechanism,
        req.payments_flow,
        req.payment_terms,
        req.participant_rights,
        req.no_privity_with_borrower,
        req.participant_has_voting_rights,
        req.borrower_consent_required,
        req.borrower_consent_obtained,
        req.borrower_knowledge_of_participation,
        req.agent_notification_required,
        req.agent_notified,
        req.pro_rata_sharing,
        req.principal_sharing_percentage,
        req.interest_sharing_percentage,
        req.fee_sharing_percentage,
        req.grantor_pays_participant,
        req.settlement_frequency,
        req.participant_credit_risk,
        req.participant_funding_risk,
        req.setoff_rights,
        req.netting_provisions,
        req.grantor_representations,
        req.participant_representations,
        req.participant_assignment_rights,
        req.requires_grantor_consent_to_assign,
        req.is_sub_participation,
        req.parent_participation_id,
        req.confidential_agreement,
        req.disclosure_restrictions,
        req.trade_date,
        req.settlement_date,
        req.effective_date,
        req.termination_date,
        req.termination_conditions,
        req.participant_kyc_completed,
        req.participant_kyc_date,
        req.participant_eligible,
        req.agreement_status,
        req.cash_settled,
        req.participation_active,
        req.total_payments_received,
        req.total_payments_to_participant,
        req.notes,
        req.special_terms,
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

/// Get participation agreement by ID
#[instrument(skip(pool))]
pub async fn get_participation_agreement_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<ParticipationAgreement>, DbError> {
    let record = sqlx::query_as!(
        ParticipationAgreement,
        r#"
        SELECT
            id, document_id, credit_agreement_id, trade_id, doc_hash, participation_type,
            grantor, grantor_entity_id, grantor_remains_lender_of_record, participant,
            participant_entity_id, borrower_name, facility_reference, participation_percentage,
            participation_amount, grantor_total_commitment, grantor_outstanding_principal,
            purchase_price, purchase_price_percentage, funded_at_closing, funding_mechanism,
            payments_flow, payment_terms, participant_rights, no_privity_with_borrower,
            participant_has_voting_rights, borrower_consent_required, borrower_consent_obtained,
            borrower_knowledge_of_participation, agent_notification_required, agent_notified,
            pro_rata_sharing, principal_sharing_percentage, interest_sharing_percentage,
            fee_sharing_percentage, grantor_pays_participant, settlement_frequency,
            participant_credit_risk, participant_funding_risk, setoff_rights, netting_provisions,
            grantor_representations, participant_representations, participant_assignment_rights,
            requires_grantor_consent_to_assign, is_sub_participation, parent_participation_id,
            confidential_agreement, disclosure_restrictions, trade_date, settlement_date,
            effective_date, termination_date, termination_conditions, participant_kyc_completed,
            participant_kyc_date, participant_eligible, agreement_status, cash_settled,
            participation_active, total_payments_received, total_payments_to_participant,
            notes, special_terms, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM participation_agreements WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// Get participation agreement by document ID
#[instrument(skip(pool))]
pub async fn get_participation_agreement_by_document_id(
    pool: &PgPool,
    document_id: &str,
) -> Result<Option<ParticipationAgreement>, DbError> {
    let record = sqlx::query_as!(
        ParticipationAgreement,
        r#"
        SELECT
            id, document_id, credit_agreement_id, trade_id, doc_hash, participation_type,
            grantor, grantor_entity_id, grantor_remains_lender_of_record, participant,
            participant_entity_id, borrower_name, facility_reference, participation_percentage,
            participation_amount, grantor_total_commitment, grantor_outstanding_principal,
            purchase_price, purchase_price_percentage, funded_at_closing, funding_mechanism,
            payments_flow, payment_terms, participant_rights, no_privity_with_borrower,
            participant_has_voting_rights, borrower_consent_required, borrower_consent_obtained,
            borrower_knowledge_of_participation, agent_notification_required, agent_notified,
            pro_rata_sharing, principal_sharing_percentage, interest_sharing_percentage,
            fee_sharing_percentage, grantor_pays_participant, settlement_frequency,
            participant_credit_risk, participant_funding_risk, setoff_rights, netting_provisions,
            grantor_representations, participant_representations, participant_assignment_rights,
            requires_grantor_consent_to_assign, is_sub_participation, parent_participation_id,
            confidential_agreement, disclosure_restrictions, trade_date, settlement_date,
            effective_date, termination_date, termination_conditions, participant_kyc_completed,
            participant_kyc_date, participant_eligible, agreement_status, cash_settled,
            participation_active, total_payments_received, total_payments_to_participant,
            notes, special_terms, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        FROM participation_agreements WHERE document_id = $1
        "#,
        document_id
    )
    .fetch_optional(pool)
    .await?;

    Ok(record)
}

/// List participation agreements with pagination and filtering
#[instrument(skip(pool))]
pub async fn list_participation_agreements(
    pool: &PgPool,
    limit: i64,
    offset: i64,
    credit_agreement_id: Option<Uuid>,
    participation_type: Option<String>,
    agreement_status: Option<String>,
    participation_active: Option<bool>,
    sort_by: Option<String>,
    sort_order: Option<String>,
) -> Result<(Vec<ParticipationAgreement>, i64), DbError> {
    // Build the WHERE clause dynamically
    let mut where_clauses = Vec::new();
    let mut param_index = 3;

    if credit_agreement_id.is_some() {
        where_clauses.push(format!("credit_agreement_id = ${}", param_index));
        param_index += 1;
    }
    if participation_type.is_some() {
        where_clauses.push(format!("participation_type = ${}", param_index));
        param_index += 1;
    }
    if agreement_status.is_some() {
        where_clauses.push(format!("agreement_status = ${}", param_index));
        param_index += 1;
    }
    if participation_active.is_some() {
        where_clauses.push(format!("participation_active = ${}", param_index));
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
        Some("participation_type") => "participation_type",
        Some("agreement_status") => "agreement_status",
        _ => "created_at",
    };

    let order = match sort_order.as_deref() {
        Some("ASC") | Some("asc") => "ASC",
        _ => "DESC",
    };

    // Get total count
    let count_query = format!(
        "SELECT COUNT(*) as count FROM participation_agreements {}",
        where_clause
    );

    let mut count_query = sqlx::query_scalar(&count_query);
    count_query = count_query.bind(&credit_agreement_id);
    if participation_type.is_some() {
        count_query = count_query.bind(&participation_type);
    }
    if agreement_status.is_some() {
        count_query = count_query.bind(&agreement_status);
    }
    if participation_active.is_some() {
        count_query = count_query.bind(&participation_active);
    }

    let total: i64 = count_query.fetch_one(pool).await?;

    // Get records
    let query = format!(
        "SELECT * FROM participation_agreements {} ORDER BY {} {} LIMIT $1 OFFSET $2",
        where_clause, sort_column, order
    );

    let mut records_query = sqlx::query_as::<_, ParticipationAgreement>(&query);
    records_query = records_query.bind(limit).bind(offset);
    records_query = records_query.bind(&credit_agreement_id);
    if participation_type.is_some() {
        records_query = records_query.bind(&participation_type);
    }
    if agreement_status.is_some() {
        records_query = records_query.bind(&agreement_status);
    }
    if participation_active.is_some() {
        records_query = records_query.bind(&participation_active);
    }

    let records = records_query.fetch_all(pool).await?;

    Ok((records, total))
}

/// Update participation agreement
#[instrument(skip(pool))]
pub async fn update_participation_agreement(
    pool: &PgPool,
    id: Uuid,
    req: UpdateParticipationAgreementRequest,
    updated_by: Option<Uuid>,
) -> Result<ParticipationAgreement, DbError> {
    // Get existing record
    let existing = get_participation_agreement_by_id(pool, id)
        .await?
        .ok_or_else(|| DbError::NotFound(format!("Participation agreement {} not found", id)))?;

    // Apply updates (use provided values or keep existing)
    let record = sqlx::query_as!(
        ParticipationAgreement,
        r#"
        UPDATE participation_agreements SET
            document_id = $2,
            credit_agreement_id = $3,
            trade_id = $4,
            doc_hash = $5,
            participation_type = $6,
            grantor = $7,
            grantor_entity_id = $8,
            grantor_remains_lender_of_record = $9,
            participant = $10,
            participant_entity_id = $11,
            borrower_name = $12,
            facility_reference = $13,
            participation_percentage = $14,
            participation_amount = $15,
            grantor_total_commitment = $16,
            grantor_outstanding_principal = $17,
            purchase_price = $18,
            purchase_price_percentage = $19,
            funded_at_closing = $20,
            funding_mechanism = $21,
            payments_flow = $22,
            payment_terms = $23,
            participant_rights = $24,
            no_privity_with_borrower = $25,
            participant_has_voting_rights = $26,
            borrower_consent_required = $27,
            borrower_consent_obtained = $28,
            borrower_knowledge_of_participation = $29,
            agent_notification_required = $30,
            agent_notified = $31,
            pro_rata_sharing = $32,
            principal_sharing_percentage = $33,
            interest_sharing_percentage = $34,
            fee_sharing_percentage = $35,
            grantor_pays_participant = $36,
            settlement_frequency = $37,
            participant_credit_risk = $38,
            participant_funding_risk = $39,
            setoff_rights = $40,
            netting_provisions = $41,
            grantor_representations = $42,
            participant_representations = $43,
            participant_assignment_rights = $44,
            requires_grantor_consent_to_assign = $45,
            is_sub_participation = $46,
            parent_participation_id = $47,
            confidential_agreement = $48,
            disclosure_restrictions = $49,
            trade_date = $50,
            settlement_date = $51,
            effective_date = $52,
            termination_date = $53,
            termination_conditions = $54,
            participant_kyc_completed = $55,
            participant_kyc_date = $56,
            participant_eligible = $57,
            agreement_status = $58,
            cash_settled = $59,
            participation_active = $60,
            total_payments_received = $61,
            total_payments_to_participant = $62,
            notes = $63,
            special_terms = $64,
            metadata = $65,
            updated_by = $66
        WHERE id = $1
        RETURNING
            id, document_id, credit_agreement_id, trade_id, doc_hash, participation_type,
            grantor, grantor_entity_id, grantor_remains_lender_of_record, participant,
            participant_entity_id, borrower_name, facility_reference, participation_percentage,
            participation_amount, grantor_total_commitment, grantor_outstanding_principal,
            purchase_price, purchase_price_percentage, funded_at_closing, funding_mechanism,
            payments_flow, payment_terms, participant_rights, no_privity_with_borrower,
            participant_has_voting_rights, borrower_consent_required, borrower_consent_obtained,
            borrower_knowledge_of_participation, agent_notification_required, agent_notified,
            pro_rata_sharing, principal_sharing_percentage, interest_sharing_percentage,
            fee_sharing_percentage, grantor_pays_participant, settlement_frequency,
            participant_credit_risk, participant_funding_risk, setoff_rights, netting_provisions,
            grantor_representations, participant_representations, participant_assignment_rights,
            requires_grantor_consent_to_assign, is_sub_participation, parent_participation_id,
            confidential_agreement, disclosure_restrictions, trade_date, settlement_date,
            effective_date, termination_date, termination_conditions, participant_kyc_completed,
            participant_kyc_date, participant_eligible, agreement_status, cash_settled,
            participation_active, total_payments_received, total_payments_to_participant,
            notes, special_terms, metadata,
            created_at as "created_at!", updated_at as "updated_at!",
            created_by, updated_by
        "#,
        id,
        req.document_id.unwrap_or(existing.document_id),
        req.credit_agreement_id.or(existing.credit_agreement_id),
        req.trade_id.or(existing.trade_id),
        req.doc_hash.unwrap_or(existing.doc_hash),
        req.participation_type
            .unwrap_or(existing.participation_type),
        req.grantor.unwrap_or(existing.grantor),
        req.grantor_entity_id.or(existing.grantor_entity_id),
        req.grantor_remains_lender_of_record
            .or(existing.grantor_remains_lender_of_record),
        req.participant.unwrap_or(existing.participant),
        req.participant_entity_id.or(existing.participant_entity_id),
        req.borrower_name.unwrap_or(existing.borrower_name),
        req.facility_reference.or(existing.facility_reference),
        req.participation_percentage
            .or(existing.participation_percentage),
        req.participation_amount.or(existing.participation_amount),
        req.grantor_total_commitment
            .or(existing.grantor_total_commitment),
        req.grantor_outstanding_principal
            .or(existing.grantor_outstanding_principal),
        req.purchase_price.or(existing.purchase_price),
        req.purchase_price_percentage
            .or(existing.purchase_price_percentage),
        req.funded_at_closing.or(existing.funded_at_closing),
        req.funding_mechanism.or(existing.funding_mechanism),
        req.payments_flow.unwrap_or(existing.payments_flow),
        req.payment_terms.or(existing.payment_terms),
        req.participant_rights.or(existing.participant_rights),
        req.no_privity_with_borrower
            .or(existing.no_privity_with_borrower),
        req.participant_has_voting_rights
            .or(existing.participant_has_voting_rights),
        req.borrower_consent_required
            .or(existing.borrower_consent_required),
        req.borrower_consent_obtained
            .or(existing.borrower_consent_obtained),
        req.borrower_knowledge_of_participation
            .or(existing.borrower_knowledge_of_participation),
        req.agent_notification_required
            .or(existing.agent_notification_required),
        req.agent_notified.or(existing.agent_notified),
        req.pro_rata_sharing.or(existing.pro_rata_sharing),
        req.principal_sharing_percentage
            .or(existing.principal_sharing_percentage),
        req.interest_sharing_percentage
            .or(existing.interest_sharing_percentage),
        req.fee_sharing_percentage
            .or(existing.fee_sharing_percentage),
        req.grantor_pays_participant
            .or(existing.grantor_pays_participant),
        req.settlement_frequency.or(existing.settlement_frequency),
        req.participant_credit_risk
            .or(existing.participant_credit_risk),
        req.participant_funding_risk
            .or(existing.participant_funding_risk),
        req.setoff_rights.or(existing.setoff_rights),
        req.netting_provisions.or(existing.netting_provisions),
        req.grantor_representations
            .or(existing.grantor_representations),
        req.participant_representations
            .or(existing.participant_representations),
        req.participant_assignment_rights
            .or(existing.participant_assignment_rights),
        req.requires_grantor_consent_to_assign
            .or(existing.requires_grantor_consent_to_assign),
        req.is_sub_participation.or(existing.is_sub_participation),
        req.parent_participation_id
            .or(existing.parent_participation_id),
        req.confidential_agreement
            .or(existing.confidential_agreement),
        req.disclosure_restrictions
            .or(existing.disclosure_restrictions),
        req.trade_date.unwrap_or(existing.trade_date),
        req.settlement_date.unwrap_or(existing.settlement_date),
        req.effective_date.unwrap_or(existing.effective_date),
        req.termination_date.or(existing.termination_date),
        req.termination_conditions
            .or(existing.termination_conditions),
        req.participant_kyc_completed
            .or(existing.participant_kyc_completed),
        req.participant_kyc_date.or(existing.participant_kyc_date),
        req.participant_eligible.or(existing.participant_eligible),
        req.agreement_status.or(existing.agreement_status),
        req.cash_settled.or(existing.cash_settled),
        req.participation_active.or(existing.participation_active),
        req.total_payments_received
            .or(existing.total_payments_received),
        req.total_payments_to_participant
            .or(existing.total_payments_to_participant),
        req.notes.or(existing.notes),
        req.special_terms.or(existing.special_terms),
        req.metadata.or(existing.metadata),
        updated_by,
    )
    .fetch_one(pool)
    .await?;

    Ok(record)
}

/// Delete participation agreement
#[instrument(skip(pool))]
pub async fn delete_participation_agreement(pool: &PgPool, id: Uuid) -> Result<bool, DbError> {
    let result = sqlx::query!(
        r#"
        DELETE FROM participation_agreements WHERE id = $1
        "#,
        id
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create multiple participation agreements in a batch
#[instrument(skip(pool, requests))]
pub async fn create_participation_agreements_batch(
    pool: &PgPool,
    requests: Vec<CreateParticipationAgreementRequest>,
) -> Result<Vec<Result<ParticipationAgreement, DbError>>, DbError> {
    let mut results = Vec::new();

    for req in requests {
        let result = create_participation_agreement(pool, req).await;
        results.push(result);
    }

    Ok(results)
}
