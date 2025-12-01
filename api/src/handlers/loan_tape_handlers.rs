///! Loan Tape API handlers
///!
///! This module provides endpoints for loan tape operations, which combine
///! deal data with payment statistics, company information, and computed metrics.
///!
///! Endpoints:
///! - POST /api/v1/loan-tape/bulk - Upload loan tape portfolio (CSV → JSON)
///! - GET /api/v1/loan-tape - List loan tape records with pagination
///! - GET /api/v1/loan-tape/{id} - Get single loan tape record
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{error, info, instrument, warn};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ApiState;
use crate::dto::loan_tape_dto::{BulkCreateLoanTapeRequest, LoanTapeListResponse, LoanTapeRecord};
use crate::handlers::deals::trigger_metrics_refresh_async;
use crate::middleware::session_middleware::extract_current_user;
use arda_credit_db::{
    business::company::{
        create_company_with_membership, get_company_by_name, get_user_membership, ProfileType,
    },
    count_loan_tape_records, create_collateral, create_loan_tape_record, get_identity_by_id,
    get_loan_tape_record_by_id, get_loan_tape_records, CreateCollateralRequest,
};

// ============================================================================
// REQUEST / RESPONSE TYPES
// ============================================================================

/// Query parameters for listing loan tape records
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListLoanTapeQuery {
    /// Filter by deal status (optional)
    #[param(example = "active")]
    pub status: Option<String>,

    /// Maximum number of records to return
    #[param(example = 20)]
    pub limit: Option<i64>,

    /// Number of records to skip for pagination
    #[param(example = 0)]
    pub offset: Option<i64>,
}

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
    pub records: Vec<LoanTapeRecord>,
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
    let mut created_records = Vec::new();

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
                    warn!(
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

        let deal = create_loan_tape_record(
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

        if let Some(record) = get_loan_tape_record_by_id(&state.pool, deal.id)
            .await
            .ok()
            .flatten()
        {
            created_records.push(convert_db_record_to_dto(record));
        } else {
            warn!(
                "Created deal but failed to fetch full record: deal_id={}",
                deal.id
            );
        }
    }

    info!(
        "Successfully created {} loan tape records for user {} ({})",
        created_records.len(),
        user.email,
        consistent_role
    );

    // Trigger async metrics refresh after bulk deal creation
    trigger_metrics_refresh_async(&state);

    Ok((
        StatusCode::CREATED,
        Json(BulkLoanTapeResponse {
            success: true,
            created_count: created_records.len(),
            records: created_records,
        }),
    ))
}

/// List loan tape records with pagination and filtering
///
/// Retrieves loan tape records with full detail including:
/// - Company information (originator/investor names and KYB status)
/// - Computed metrics (APR, days to maturity)
/// - Payment statistics (aggregated from deal_payments table)
///
/// # Query Parameters
/// - `status`: Filter by deal status (e.g., "active", "paid_off", "closed")
/// - `limit`: Maximum number of records to return (default: 20, max: 100)
/// - `offset`: Number of records to skip for pagination (default: 0)
///
/// # Authentication
/// Requires valid session token via Authorization header.
#[utoipa::path(
    get,
    path = "/api/v1/loan-tape",
    params(ListLoanTapeQuery),
    responses(
        (status = 200, description = "Loan tape records retrieved successfully", body = LoanTapeListResponse,
            example = json!({
                "records": [
                    {
                        "loan_id": "550e8400-e29b-41d4-a716-446655440000",
                        "deal_id": "DL-20240115-001",
                        "originator_name": "TechFlow Solutions Inc",
                        "investor_name": "Apex Credit Partners LP",
                        "principal_amount": "2500000.00",
                        "outstanding_balance": "2150000.00",
                        "interest_rate_bps": 875,
                        "interest_rate_apr": "8.75",
                        "loan_status": "active",
                        "total_payments_made": 10
                    }
                ],
                "total": 45,
                "count": 20,
                "offset": 0,
                "limit": 20
            })
        ),
        (status = 401, description = "Unauthorized - authentication required", body = ErrorResponse,
            example = json!({
                "error": "Unauthorized",
                "details": "Missing or invalid session token"
            })
        ),
        (status = 500, description = "Internal server error", body = ErrorResponse,
            example = json!({
                "error": "Failed to fetch records",
                "details": "Database query failed"
            })
        ),
    ),
    security(
        ("bearer" = [])
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state, headers))]
pub async fn list_loan_tape_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Query(query): Query<ListLoanTapeQuery>,
) -> Result<Json<LoanTapeListResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Authenticate user
    let _user = extract_current_user(&headers).ok_or_else(|| {
        error!("Authentication failed: missing or invalid session");
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
                details: Some("Missing or invalid session token".to_string()),
            }),
        )
    })?;

    let limit = query.limit.unwrap_or(20).min(100); // Cap at 100
    let offset = query.offset.unwrap_or(0);

    info!(
        "Listing loan tape records: status={:?}, limit={}, offset={}",
        query.status, limit, offset
    );

    // Get total count
    let total = count_loan_tape_records(&state.pool, query.status.as_deref())
        .await
        .map_err(|e| {
            error!("Failed to count loan tape records: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to count records".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    // Get records
    let records = get_loan_tape_records(&state.pool, query.status.as_deref(), limit, offset)
        .await
        .map_err(|e| {
            error!("Failed to fetch loan tape records: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch records".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    let dto_records: Vec<LoanTapeRecord> =
        records.into_iter().map(convert_db_record_to_dto).collect();

    Ok(Json(LoanTapeListResponse {
        records: dto_records.clone(),
        total,
        count: dto_records.len(),
        offset,
        limit,
    }))
}

/// Get a single loan tape record by ID
///
/// Retrieves a complete loan tape record with all joined and computed fields including:
/// - Full company details (names, KYB status)
/// - Financial metrics (APR, days to maturity)
/// - Payment history (last payment, total payments, interest paid)
/// - Risk assessment fields (credit score, DSCR, LTV ratio)
/// - Blockchain tracking (state hash, proof inclusion)
///
/// # Path Parameters
/// - `id`: Deal UUID to retrieve
///
/// # Authentication
/// Requires valid session token via Authorization header.
#[utoipa::path(
    get,
    path = "/api/v1/loan-tape/{id}",
    params(
        ("id" = Uuid, Path, description = "Deal UUID", example = "550e8400-e29b-41d4-a716-446655440000")
    ),
    responses(
        (status = 200, description = "Loan tape record retrieved successfully", body = LoanTapeRecord,
            example = json!({
                "loan_id": "550e8400-e29b-41d4-a716-446655440000",
                "deal_id": "DL-20240115-001",
                "originator_id": "660e8400-e29b-41d4-a716-446655440001",
                "originator_name": "TechFlow Solutions Inc",
                "originator_kyb_status": "approved",
                "investor_id": "770e8400-e29b-41d4-a716-446655440002",
                "investor_name": "Apex Credit Partners LP",
                "investor_kyb_status": "approved",
                "principal_amount": "2500000.00",
                "outstanding_balance": "2150000.00",
                "interest_rate_bps": 875,
                "interest_rate_apr": "8.75",
                "loan_status": "active",
                "originated_at": "2024-01-15T09:30:00Z",
                "maturity_date": "2027-01-15T00:00:00Z",
                "days_to_maturity": 428,
                "payment_frequency": "monthly",
                "last_payment_date": "2024-11-15T00:00:00Z",
                "last_payment_amount": "45833.33",
                "total_payments_made": 10,
                "total_interest_paid": "287500.00",
                "collateral_type": "Equipment",
                "collateral_value": "3500000.00",
                "ltv_ratio": "0.7143",
                "credit_score": 720,
                "dscr": "1.45",
                "risk_level": "medium",
                "asset_class": "direct_lending",
                "region": "north_america",
                "borrower_industry": "software_saas",
                "deal_title": "TechFlow Working Capital Facility",
                "state_transition_hash": "0x7f8e9a2b1c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f",
                "proof_included": true,
                "created_at": "2024-01-15T09:30:00Z",
                "updated_at": "2024-11-15T10:00:00Z"
            })
        ),
        (status = 401, description = "Unauthorized - authentication required", body = ErrorResponse,
            example = json!({
                "error": "Unauthorized",
                "details": "Missing or invalid session token"
            })
        ),
        (status = 404, description = "Loan tape record not found", body = ErrorResponse,
            example = json!({
                "error": "Loan tape record not found",
                "details": "No record found with ID: 550e8400-e29b-41d4-a716-446655440000"
            })
        ),
        (status = 500, description = "Internal server error", body = ErrorResponse,
            example = json!({
                "error": "Failed to fetch record",
                "details": "Database query failed"
            })
        ),
    ),
    security(
        ("bearer" = [])
    ),
    tag = "Loan Tape"
)]
#[instrument(skip(state, headers))]
pub async fn get_loan_tape_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<LoanTapeRecord>, (StatusCode, Json<ErrorResponse>)> {
    // Authenticate user
    let _user = extract_current_user(&headers).ok_or_else(|| {
        error!("Authentication failed: missing or invalid session");
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Unauthorized".to_string(),
                details: Some("Missing or invalid session token".to_string()),
            }),
        )
    })?;

    info!("Fetching loan tape record: id={}", id);

    let record = get_loan_tape_record_by_id(&state.pool, id)
        .await
        .map_err(|e| {
            error!("Failed to fetch loan tape record: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch record".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Loan tape record not found".to_string(),
                    details: Some(format!("No record found with ID: {}", id)),
                }),
            )
        })?;

    let dto_record = convert_db_record_to_dto(record);

    Ok(Json(dto_record))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Convert database LoanTapeRecord to DTO LoanTapeRecord
fn convert_db_record_to_dto(record: arda_credit_db::LoanTapeRecord) -> LoanTapeRecord {
    LoanTapeRecord {
        loan_id: record.loan_id,
        deal_id: record.deal_id,
        originator_id: record.originator_id,
        originator_name: record.originator_name,
        originator_kyb_status: record.originator_kyb_status,
        investor_id: record.investor_id,
        investor_name: record.investor_name,
        investor_kyb_status: record.investor_kyb_status,
        principal_amount: record.principal_amount,
        outstanding_balance: record.outstanding_balance,
        interest_rate_bps: record.interest_rate_bps,
        interest_rate_apr: record.interest_rate_apr,
        loan_status: record.loan_status,
        originated_at: record.originated_at,
        maturity_date: record.maturity_date,
        days_to_maturity: record.days_to_maturity,
        payment_frequency: record.payment_frequency,
        last_payment_date: record.last_payment_date,
        last_payment_amount: record.last_payment_amount,
        total_payments_made: record.total_payments_made,
        total_interest_paid: record.total_interest_paid,
        collateral_type: record.collateral_type,
        collateral_value: record.collateral_value,
        ltv_ratio: record.ltv_ratio,
        risk_level: record.risk_level,
        asset_class: record.asset_class,
        region: record.region,
        borrower_industry: record.borrower_industry,
        credit_score: record.credit_score,
        dscr: record.dscr,
        deal_title: record.deal_title,
        state_transition_hash: record
            .state_transition_hash
            .map(|h| format!("0x{}", hex::encode(h))),
        proof_included: record.proof_included,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}
