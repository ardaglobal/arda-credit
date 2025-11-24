//! Document extraction handler for deal creation from OCR-extracted company data
//!
//! This module handles the workflow where:
//! 1. Frontend sends OCR-extracted company data to backend
//! 2. Backend creates or looks up all companies (originator, investor, borrower)
//! 3. User selects which company they belong to
//! 4. Backend associates user with that company as admin
//! 5. Backend returns pre-filled deal data for frontend to display

use axum::{extract::State, http::StatusCode, response::Json};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::config::types::ApiState;
use crate::middleware::session_middleware::extract_current_user;
use arda_credit_db::business::company::{
    create_membership, get_or_create_company, get_user_membership, accept_membership, CompanyRole,
    ProfileType,
};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Company extraction data from OCR
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "legal_name": "Acme Corp",
    "industry": "Technology",
    "jurisdiction": "Delaware"
}))]
pub struct CompanyExtractionData {
    /// Legal name of the company (required)
    #[schema(example = "Acme Corp")]
    pub legal_name: String,

    /// Industry sector (optional)
    #[schema(example = "Technology")]
    pub industry: Option<String>,

    /// Jurisdiction (optional)
    #[schema(example = "Delaware")]
    pub jurisdiction: Option<String>,

    /// Additional company metadata from OCR (optional)
    pub metadata: Option<serde_json::Value>,
}

/// User association request - which company the user belongs to
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "company_role": "originator",
    "profile_type": "originator"
}))]
pub struct UserAssociationRequest {
    /// Which company role the user belongs to: "originator", "investor", or "borrower"
    #[schema(example = "originator")]
    pub company_role: String, // "originator" | "investor" | "borrower"

    /// User's profile type: "originator" or "investor"
    #[schema(example = "originator")]
    pub profile_type: String, // "originator" | "investor"
}

/// Optional deal terms extracted from document for pre-filling
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "principal_amount": "100000.00",
    "interest_rate_bps": 500,
    "term_days": 365
}))]
pub struct DealTermsData {
    /// Principal amount (optional)
    #[schema(example = "100000.00")]
    pub principal_amount: Option<String>,

    /// Interest rate in basis points (optional)
    #[schema(example = 500)]
    pub interest_rate_bps: Option<u16>,

    /// Term in days (optional)
    #[schema(example = 365)]
    pub term_days: Option<u32>,

    /// Additional deal metadata (optional)
    pub metadata: Option<serde_json::Value>,
}

/// Request to process document extraction and create/associate companies
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "document_reference": "https://ocr-service.com/documents/abc123",
    "originator": {
        "legal_name": "Acme Corp",
        "industry": "Technology",
        "jurisdiction": "Delaware"
    },
    "investor": {
        "legal_name": "Investment Fund LLC",
        "industry": "Finance"
    },
    "borrower": {
        "legal_name": "BorrowerCo Inc"
    },
    "user_association": {
        "company_role": "originator",
        "profile_type": "originator"
    },
    "deal_terms": {
        "principal_amount": "100000.00",
        "interest_rate_bps": 500,
        "term_days": 365
    }
}))]
pub struct DocumentExtractionRequest {
    /// Reference URL or ID to the document in the OCR service (optional)
    #[schema(example = "https://ocr-service.com/documents/abc123")]
    pub document_reference: Option<String>,

    /// Originator company data (required)
    pub originator: CompanyExtractionData,

    /// Investor company data (required)
    pub investor: CompanyExtractionData,

    /// Borrower company data (optional third party)
    pub borrower: Option<CompanyExtractionData>,

    /// User association - which company the user belongs to
    pub user_association: UserAssociationRequest,

    /// Optional deal terms for pre-filling (optional)
    pub deal_terms: Option<DealTermsData>,
}

/// Company creation result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CompanyResult {
    /// Company UUID
    pub id: String,

    /// Legal name of the company
    pub legal_name: String,

    /// Whether the company was newly created (true) or already existed (false)
    pub was_created: bool,
}

/// User membership result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UserMembershipResult {
    /// Company UUID that user was associated with
    pub company_id: String,

    /// Company legal name
    pub company_name: String,

    /// User's role in the company
    pub role: String,

    /// User's profile type
    pub profile_type: String,

    /// Membership status
    pub status: String,
}

/// Pre-filled deal data for frontend
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PreFilledDealData {
    /// Originator company ID
    pub originator_id: String,

    /// Investor company ID
    pub investor_id: String,

    /// Borrower company ID (optional)
    pub borrower_id: Option<String>,

    /// Principal amount (optional)
    pub principal_amount: Option<String>,

    /// Interest rate in basis points (optional)
    pub interest_rate_bps: Option<u16>,

    /// Term in days (optional)
    pub term_days: Option<u32>,

    /// Document reference (optional)
    pub document_reference: Option<String>,
}

/// Response from document extraction processing
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DocumentExtractionResponse {
    /// Companies that were created or looked up
    pub companies: CompaniesResult,

    /// User membership that was created or updated
    pub user_membership: UserMembershipResult,

    /// Pre-filled deal data ready for frontend to display
    pub pre_filled_deal: PreFilledDealData,
}

/// All companies involved in the deal
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CompaniesResult {
    /// Originator company
    pub originator: CompanyResult,

    /// Investor company
    pub investor: CompanyResult,

    /// Borrower company (optional)
    pub borrower: Option<CompanyResult>,
}

// ============================================================================
// HANDLER
// ============================================================================

/// Process document extraction data to create companies, associate user, and return pre-filled deal
///
/// This endpoint accepts OCR-extracted company data from the frontend and:
/// 1. Creates or looks up all companies (originator, investor, borrower)
/// 2. Associates the authenticated user with the selected company as admin
/// 3. Returns pre-filled deal data for the frontend to display
///
/// The user must be authenticated to use this endpoint.
#[utoipa::path(
    post,
    path = "/api/v1/deals/process-document-extraction",
    request_body = DocumentExtractionRequest,
    responses(
        (status = 200, description = "Document extraction processed successfully", body = DocumentExtractionResponse),
        (status = 400, description = "Invalid request - check field validation"),
        (status = 401, description = "Unauthorized - user must be authenticated"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals"
)]
#[instrument(skip(state, headers), level = "info")]
pub async fn process_document_extraction(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(request): Json<DocumentExtractionRequest>,
) -> Result<Json<DocumentExtractionResponse>, (StatusCode, String)> {
    info!("📄 Processing document extraction for companies");

    // Extract authenticated user from session
    let current_user = extract_current_user(&headers)
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Authentication required".to_string(),
        ))?;

    info!(
        "👤 User {} processing document extraction",
        current_user.id
    );

    // Validate profile_type
    let profile_type = match request.user_association.profile_type.to_lowercase().as_str() {
        "originator" => ProfileType::Originator,
        "investor" => ProfileType::Investor,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "Invalid profile_type '{}'. Valid values: originator, investor",
                    request.user_association.profile_type
                ),
            ))
        }
    };

    // Validate company_role
    let company_role = request.user_association.company_role.to_lowercase();
    if !["originator", "investor", "borrower"].contains(&company_role.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Invalid company_role '{}'. Valid values: originator, investor, borrower",
                request.user_association.company_role
            ),
        ));
    }

    // Step 1: Create or lookup originator company
    info!(
        "🏢 Creating/looking up originator company: {}",
        request.originator.legal_name
    );
    let originator_uuid = Uuid::new_v4(); // Will be used if company doesn't exist
    let originator_company = get_or_create_company(
        &state.pool,
        originator_uuid,
        &request.originator.legal_name,
    )
    .await
    .map_err(|e| {
        error!("❌ Failed to create/lookup originator company: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to create/lookup originator company: {}", e),
        )
    })?;

    let originator_was_created = originator_company.id == originator_uuid;
    info!(
        "✅ Originator company: {} ({})",
        originator_company.id,
        if originator_was_created {
            "created"
        } else {
            "existing"
        }
    );

    // Step 2: Create or lookup investor company
    info!(
        "🏢 Creating/looking up investor company: {}",
        request.investor.legal_name
    );
    let investor_uuid = Uuid::new_v4();
    let investor_company =
        get_or_create_company(&state.pool, investor_uuid, &request.investor.legal_name)
            .await
            .map_err(|e| {
                error!("❌ Failed to create/lookup investor company: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create/lookup investor company: {}", e),
                )
            })?;

    let investor_was_created = investor_company.id == investor_uuid;
    info!(
        "✅ Investor company: {} ({})",
        investor_company.id,
        if investor_was_created {
            "created"
        } else {
            "existing"
        }
    );

    // Step 3: Create or lookup borrower company (optional)
    let borrower_result = if let Some(ref borrower_data) = request.borrower {
        info!(
            "🏢 Creating/looking up borrower company: {}",
            borrower_data.legal_name
        );
        let borrower_uuid = Uuid::new_v4();
        let borrower_company =
            get_or_create_company(&state.pool, borrower_uuid, &borrower_data.legal_name)
                .await
                .map_err(|e| {
                    error!("❌ Failed to create/lookup borrower company: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to create/lookup borrower company: {}", e),
                    )
                })?;

        let borrower_was_created = borrower_company.id == borrower_uuid;
        info!(
            "✅ Borrower company: {} ({})",
            borrower_company.id,
            if borrower_was_created {
                "created"
            } else {
                "existing"
            }
        );

        Some(CompanyResult {
            id: borrower_company.id.to_string(),
            legal_name: borrower_data.legal_name.clone(),
            was_created: borrower_was_created,
        })
    } else {
        None
    };

    // Step 4: Determine which company to associate user with
    let (selected_company_id, selected_company_name) = match company_role.as_str() {
        "originator" => (originator_company.id, request.originator.legal_name.clone()),
        "investor" => (investor_company.id, request.investor.legal_name.clone()),
        "borrower" => {
            if let Some(ref borrower_data) = request.borrower {
                (
                    borrower_result.as_ref().unwrap().id.parse::<Uuid>().unwrap(),
                    borrower_data.legal_name.clone(),
                )
            } else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "Cannot associate with borrower - borrower data not provided".to_string(),
                ));
            }
        }
        _ => unreachable!(),
    };

    // Step 5: Create or update user membership (idempotent)
    info!(
        "👥 Associating user {} with company {} as admin",
        current_user.id, selected_company_id
    );

    let membership = match get_user_membership(
        &state.pool,
        selected_company_id,
        current_user.id,
        profile_type,
    )
    .await
    {
        Ok(existing_membership) => {
            info!("✅ User already has membership in company (idempotent)");
            existing_membership
        }
        Err(_) => {
            // Create new membership
            info!("➕ Creating new admin membership for user");
            let new_membership = create_membership(
                &state.pool,
                selected_company_id,
                current_user.id,
                CompanyRole::Admin,
                profile_type,
                None, // No inviter (self-association)
                None, // Use default admin permissions
            )
            .await
            .map_err(|e| {
                error!("❌ Failed to create membership: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to create membership: {}", e),
                )
            })?;

            // Immediately activate the membership
            accept_membership(&state.pool, new_membership.id)
                .await
                .map_err(|e| {
                    error!("❌ Failed to activate membership: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Failed to activate membership: {}", e),
                    )
                })?
        }
    };

    info!(
        "✅ User membership: {} (status: {})",
        membership.id, membership.status
    );

    // Step 6: Build pre-filled deal data
    let pre_filled_deal = PreFilledDealData {
        originator_id: originator_company.id.to_string(),
        investor_id: investor_company.id.to_string(),
        borrower_id: borrower_result.as_ref().map(|b| b.id.clone()),
        principal_amount: request
            .deal_terms
            .as_ref()
            .and_then(|t| t.principal_amount.clone()),
        interest_rate_bps: request.deal_terms.as_ref().and_then(|t| t.interest_rate_bps),
        term_days: request.deal_terms.as_ref().and_then(|t| t.term_days),
        document_reference: request.document_reference.clone(),
    };

    // Step 7: Build response
    let response = DocumentExtractionResponse {
        companies: CompaniesResult {
            originator: CompanyResult {
                id: originator_company.id.to_string(),
                legal_name: request.originator.legal_name,
                was_created: originator_was_created,
            },
            investor: CompanyResult {
                id: investor_company.id.to_string(),
                legal_name: request.investor.legal_name,
                was_created: investor_was_created,
            },
            borrower: borrower_result,
        },
        user_membership: UserMembershipResult {
            company_id: selected_company_id.to_string(),
            company_name: selected_company_name,
            role: "admin".to_string(),
            profile_type: profile_type.to_string(),
            status: membership.status.to_string(),
        },
        pre_filled_deal,
    };

    info!("✅ Document extraction processed successfully");
    Ok(Json(response))
}
