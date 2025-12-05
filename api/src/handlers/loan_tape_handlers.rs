///! Loan Tape API handlers
///!
///! This module provides endpoints for loan tape operations, which combine
///! deal data with payment statistics, company information, and computed metrics.
///!
///! Endpoints:
///! - POST /api/v1/loan-tape/bulk - Upload loan tape portfolio (CSV → JSON)
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use tracing::{error, info, instrument};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::config::types::ApiState;
use crate::dto::loan_tape_dto::BulkCreateLoanTapeRequest;
use crate::handlers::deals::trigger_metrics_refresh_async;
use arda_credit_db::{
    business::company::{
        create_company_with_membership, get_company_by_name, get_user_membership, ProfileType,
    },
    create_collateral, create_loan_tape_record, get_identity_by_id, CreateCollateralRequest,
};

// ============================================================================
// REQUEST / RESPONSE TYPES
// ============================================================================

/// Generic error response
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

/// Bulk create response
#[derive(Debug, Serialize, ToSchema)]
pub struct BulkLoanTapeResponse {
    pub success: bool,
    pub created_count: usize,
    pub records: Vec<serde_json::Value>, // Empty for performance - use separate query to fetch records
}

// ============================================================================
// HANDLERS
// ============================================================================

/// Upload loan tape portfolio (bulk create)
///
/// Uploads a portfolio of deals from a CSV file (converted to JSON by client).
/// The system auto-detects the consistent party (originator or investor) and:
/// - Validates the user exists and is associated with the consistent company
/// - Creates companies as needed (consistent + counterparties)
/// - Creates all deal records
///
/// # Portfolio Requirements
/// - All records must have the same originator OR same investor (single-party portfolio)
/// - User must be a member of the consistent company (or it will be created)
///
/// # Workflow
/// 1. Validates user_id exists
/// 2. Auto-detects consistent party (originator or investor)
/// 3. Gets or creates consistent company
/// 4. Validates user membership (or creates if company is new)
/// 5. Gets or creates all counterparty companies
/// 6. Creates all deal records
///
/// # Examples
///
/// **Valid Portfolio (Originator):**
/// All records have "TechFlow Inc" as originator, different investors.
///
/// **Valid Portfolio (Investor):**
/// All records have "Apex Fund LP" as investor, different originators.
///
/// **Invalid Portfolio:**
/// Multiple unique originators AND multiple unique investors (rejected with 400).
#[utoipa::path(
    post,
    path = "/api/v1/loan-tape/bulk",
    request_body = BulkCreateLoanTapeRequest,
    responses(
        (status = 201, description = "Loan tape records created successfully", body = BulkLoanTapeResponse,
            example = json!({
                "success": true,
                "created_count": 3,
                "records": [
                    {
                        "loan_id": "550e8400-e29b-41d4-a716-446655440000",
                        "deal_id": "DL-20240115-001",
                        "originator_id": "660e8400-e29b-41d4-a716-446655440001",
                        "originator_name": "TechFlow Solutions Inc",
                        "originator_kyb_status": "approved",
                        "investor_id": "770e8400-e29b-41d4-a716-446655440002",
                        "investor_name": "Apex Credit Partners LP",
                        "investor_kyb_status": "not_submitted",
                        "principal_amount": "2500000.00",
                        "outstanding_balance": "2150000.00",
                        "interest_rate_bps": 875,
                        "interest_rate_apr": "8.75",
                        "loan_status": "active",
                        "originated_at": "2024-01-15T09:30:00Z",
                        "maturity_date": "2027-01-15T00:00:00Z"
                    }
                ]
            })
        ),
        (status = 400, description = "Invalid request - multi-party portfolio or validation failed", body = ErrorResponse,
            examples(
                ("Multi-party Portfolio" = (value = json!({
                    "error": "Multi-party portfolio not supported",
                    "details": "Upload must contain portfolio for a single party. Found 3 unique originators and 3 unique investors."
                }))),
                ("Invalid User" = (value = json!({
                    "error": "Invalid user ID",
                    "details": "User 550e8400-e29b-41d4-a716-446655440000 not found"
                }))),
                ("No Records" = (value = json!({
                    "error": "No records provided",
                    "details": "Request must contain at least one record"
                })))
            )
        ),
        (status = 403, description = "Forbidden - user not associated with consistent company", body = ErrorResponse,
            example = json!({
                "error": "User not authorized for this company",
                "details": "User must be a member of company 'TechFlow Solutions Inc' with originator profile to upload its portfolio"
            })
        ),
        (status = 500, description = "Internal server error", body = ErrorResponse,
            example = json!({
                "error": "Failed to create loan tape record",
                "details": "Database transaction failed"
            })
        ),
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state))]
pub async fn create_bulk_loan_tape_handler(
    State(state): State<ApiState>,
    Json(req): Json<BulkCreateLoanTapeRequest>,
) -> Result<(StatusCode, Json<BulkLoanTapeResponse>), (StatusCode, Json<ErrorResponse>)> {
    if req.records.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "No records provided".to_string(),
                details: Some("Request must contain at least one record".to_string()),
            }),
        ));
    }

    info!(
        "Processing loan tape upload: user_id={}, record_count={}",
        req.user_id,
        req.records.len()
    );

    // Step 1: Validate user exists
    let user = get_identity_by_id(&state.pool, req.user_id)
        .await
        .map_err(|e| {
            error!(
                "User validation failed: user_id={}, error={}",
                req.user_id, e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to lookup user".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?
        .ok_or_else(|| {
            error!("User not found: user_id={}", req.user_id);
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Invalid user ID".to_string(),
                    details: Some(format!("User {} not found", req.user_id)),
                }),
            )
        })?;

    info!("User validated: email={}", user.email);

    // Step 2: Auto-detect consistent party
    let mut originator_names: HashSet<String> = HashSet::new();
    let mut investor_names: HashSet<String> = HashSet::new();

    for record in &req.records {
        originator_names.insert(record.originator_name.clone());
        investor_names.insert(record.investor_name.clone());
    }

    let (consistent_role, consistent_name) = if originator_names.len() == 1
        && investor_names.len() > 1
    {
        (
            "originator",
            originator_names.iter().next().unwrap().clone(),
        )
    } else if investor_names.len() == 1 && originator_names.len() > 1 {
        ("investor", investor_names.iter().next().unwrap().clone())
    } else if originator_names.len() == 1 && investor_names.len() == 1 {
        // Edge case: both are consistent (single deal or same counterparty)
        // Default to originator as consistent party
        (
            "originator",
            originator_names.iter().next().unwrap().clone(),
        )
    } else {
        error!(
            "Multi-party portfolio detected: originator_count={}, investor_count={}",
            originator_names.len(),
            investor_names.len()
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Multi-party portfolio not supported".to_string(),
                details: Some(format!(
                    "Upload must contain portfolio for a single party. Found {} unique originators and {} unique investors.",
                    originator_names.len(),
                    investor_names.len()
                )),
            }),
        ));
    };

    let profile_type = if consistent_role == "originator" {
        ProfileType::Originator
    } else {
        ProfileType::Investor
    };

    info!(
        "Detected consistent party: role={}, name={}, profile_type={:?}",
        consistent_role, consistent_name, profile_type
    );

    // Step 3: Get or create consistent company and validate/create user membership
    let consistent_company = match get_company_by_name(&state.pool, &consistent_name)
        .await
        .map_err(|e| {
            error!("Failed to query company by name: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to lookup company".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })? {
        Some(company) => {
            info!(
                "Consistent company exists: id={}, name={}",
                company.id, company.name
            );

            // Verify user has membership with correct profile type
            match get_user_membership(&state.pool, company.id, req.user_id, profile_type).await {
                Ok(membership) => {
                    info!(
                        "User has valid membership: role={:?}, status={:?}",
                        membership.role, membership.status
                    );
                }
                Err(_) => {
                    error!(
                        "User {} not a member of company {} with profile {:?}",
                        req.user_id, company.id, profile_type
                    );
                    return Err((
                        StatusCode::FORBIDDEN,
                        Json(ErrorResponse {
                            error: "User not authorized for this company".to_string(),
                            details: Some(format!(
                                "User must be a member of company '{}' with {} profile to upload its portfolio",
                                consistent_name, consistent_role
                            )),
                        }),
                    ));
                }
            }

            company
        }
        None => {
            info!(
                "Consistent company does not exist, creating: name={}",
                consistent_name
            );

            // Create company and add user as admin member
            let (company, membership) = create_company_with_membership(
                &state.pool,
                &consistent_name,
                req.user_id,
                profile_type,
            )
            .await
            .map_err(|e| {
                error!("Failed to create company with membership: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to create company".to_string(),
                        details: Some(e.to_string()),
                    }),
                )
            })?;

            info!(
                "Created company and membership: company_id={}, membership_id={}, role={:?}",
                company.id, membership.id, membership.role
            );

            company
        }
    };

    // Step 4: Collect all counterparty company names and get or create them
    let counterparty_names: HashSet<String> = if consistent_role == "originator" {
        investor_names
    } else {
        originator_names
    };

    let mut company_cache: HashMap<String, Uuid> = HashMap::new();
    company_cache.insert(consistent_name.clone(), consistent_company.id);

    for name in counterparty_names {
        if name == consistent_name {
            continue; // Skip if it's the same as consistent company
        }

        let company = match get_company_by_name(&state.pool, &name).await.map_err(|e| {
            error!("Failed to query counterparty company: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to lookup counterparty company".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })? {
            Some(company) => {
                info!(
                    "Counterparty company exists: id={}, name={}",
                    company.id, company.name
                );
                company
            }
            None => {
                info!("Creating counterparty company: name={}", name);

                // Create company without user membership (orphaned company)
                let company = arda_credit_db::business::company::create_company(
                    &state.pool,
                    &arda_credit_db::business::company::CreateCompanyRequest {
                        name: name.clone(),
                        description: Some(format!(
                            "Auto-created from loan tape upload as counterparty"
                        )),
                        metadata: Some(serde_json::json!({
                            "auto_created": true,
                            "created_via": "loan_tape_counterparty"
                        })),
                    },
                )
                .await
                .map_err(|e| {
                    error!("Failed to create counterparty company: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to create counterparty company".to_string(),
                            details: Some(e.to_string()),
                        }),
                    )
                })?;

                info!(
                    "Created counterparty company: id={}, name={}",
                    company.id, company.name
                );
                company
            }
        };

        company_cache.insert(name, company.id);
    }

    info!("All companies ready: total_count={}", company_cache.len());

    // Step 5: Create all deal records
    let mut created_count = 0;

    for (idx, record_req) in req.records.iter().enumerate() {
        let originator_id = company_cache
            .get(&record_req.originator_name)
            .ok_or_else(|| {
                error!(
                    "Originator company not found in cache: {}",
                    record_req.originator_name
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Internal error".to_string(),
                        details: Some(format!(
                            "Originator company '{}' not in cache",
                            record_req.originator_name
                        )),
                    }),
                )
            })?;

        let investor_id = company_cache
            .get(&record_req.investor_name)
            .ok_or_else(|| {
                error!(
                    "Investor company not found in cache: {}",
                    record_req.investor_name
                );
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Internal error".to_string(),
                        details: Some(format!(
                            "Investor company '{}' not in cache",
                            record_req.investor_name
                        )),
                    }),
                )
            })?;

        let state_hash = record_req
            .state_transition_hash
            .as_ref()
            .and_then(|h| hex::decode(h.trim_start_matches("0x")).ok());

        // Create collateral record if collateral data is provided
        let collateral_uuid = if record_req.collateral_name.is_some()
            || record_req.collateral_type.is_some()
            || record_req.collateral_address.is_some()
        {
            // If collateral_id is provided in the request, try to parse it
            let coll_id = if let Some(id_str) = &record_req.collateral_id {
                Uuid::parse_str(id_str).ok()
            } else {
                None
            };

            // Create collateral record
            let collateral_req = CreateCollateralRequest {
                name: record_req
                    .collateral_name
                    .clone()
                    .unwrap_or_else(|| "Unnamed Collateral".to_string()),
                collateral_type: record_req.collateral_type.clone(),
                description: record_req
                    .collateral_type
                    .as_ref()
                    .map(|t| format!("Collateral of type {} imported from loan tape", t)),
                address: record_req.collateral_address.clone(),
                custom_id: record_req.collateral_custom_id.clone(),
                metadata: Some(serde_json::json!({
                    "imported_from": "loan_tape_bulk_upload",
                    "provided_id": record_req.collateral_id
                })),
                created_by: Some(req.user_id),
            };

            match create_collateral(&state.pool, collateral_req).await {
                Ok(collateral) => {
                    info!(
                        "Created collateral {} for deal at index {}",
                        collateral.id, idx
                    );
                    Some(collateral.id)
                }
                Err(e) => {
                    info!(
                        "Failed to create collateral for deal at index {}: {}. Continuing without collateral.",
                        idx, e
                    );
                    coll_id
                }
            }
        } else if let Some(id_str) = &record_req.collateral_id {
            // Only collateral_id provided, parse it
            Uuid::parse_str(id_str).ok()
        } else {
            None
        };

        create_loan_tape_record(
            &state.pool,
            *originator_id,
            Some(*investor_id),
            record_req.principal_amount,
            record_req.outstanding_balance,
            record_req.interest_rate_bps,
            &record_req.deal_status,
            record_req.originated_at,
            record_req.maturity_date,
            record_req.custom_deal_id.as_deref(),
            record_req.title.as_deref(),
            record_req.description.as_deref(),
            record_req.payment_frequency.as_deref(),
            record_req.collateral_type.as_deref(),
            record_req.collateral_value,
            record_req.ltv_ratio,
            record_req.credit_score,
            record_req.dscr,
            record_req.risk_level.as_deref(),
            record_req.asset_class.as_deref(),
            record_req.region.as_deref(),
            record_req.industry.as_deref(),
            state_hash,
            record_req.proof_included.unwrap_or(false),
            collateral_uuid,
            Some(req.document_id),
        )
        .await
        .map_err(|e| {
            error!("Failed to create loan tape record at index {}: {}", idx, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create record at index {}", idx),
                    details: Some(e.to_string()),
                }),
            )
        })?;

        created_count += 1;
    }

    info!(
        "Successfully created {} loan tape records for user {} ({})",
        created_count,
        user.email,
        consistent_role
    );

    // Trigger async metrics refresh after bulk deal creation
    trigger_metrics_refresh_async(&state);

    Ok((
        StatusCode::CREATED,
        Json(BulkLoanTapeResponse {
            success: true,
            created_count,
            records: vec![], // Records not returned in bulk upload to improve performance
        }),
    ))
}

