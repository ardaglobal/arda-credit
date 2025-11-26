//! Collateral API Handlers
//!
//! Provides REST API endpoints for collateral CRUD operations with duplicate detection.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use arda_credit_db::{
    create_collateral, create_collateral_batch, delete_collateral, get_collateral_by_custom_id,
    get_collateral_by_deal_id, get_collateral_by_id, get_collateral_with_duplicates,
    list_collateral, update_collateral, Collateral, CollateralWithDuplicates,
    CreateCollateralRequest as DbCreateRequest, UpdateCollateralRequest as DbUpdateRequest,
};

use crate::config::types::{ApiState, ErrorResponse};
use crate::handlers::taxonomy::common::{
    internal_error, not_found_error, parse_uuid, BatchError, BatchOperationResponse,
    DeleteResponse, ListQueryParams, PaginatedResponse,
};
use crate::middleware::session_middleware::extract_current_user;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Collateral API response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "123 Main Street Property",
    "collateral_type": "real_estate",
    "description": "Commercial property in downtown",
    "address": "123 Main St, New York, NY 10001",
    "custom_id": "PROP-2025-001",
    "double_pledged": null
}))]
pub struct CollateralResponse {
    pub id: String,
    pub name: String,
    pub collateral_type: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub custom_id: Option<String>,
    pub double_pledged: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<serde_json::Value>,
}

/// Collateral with duplicates response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CollateralWithDuplicatesResponse {
    pub collateral: CollateralResponse,
    pub potential_duplicates: Vec<CollateralResponse>,
}

/// Create collateral request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreateCollateralApiRequest {
    #[schema(example = "123 Main Street Property")]
    pub name: String, // Required field

    #[schema(example = "real_estate")]
    pub collateral_type: Option<String>,

    #[schema(example = "Commercial property in downtown")]
    pub description: Option<String>,

    #[schema(example = "123 Main St, New York, NY 10001")]
    pub address: Option<String>,

    #[schema(example = "PROP-2025-001")]
    pub custom_id: Option<String>,

    pub metadata: Option<serde_json::Value>,
}

/// Update collateral request
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateCollateralApiRequest {
    pub name: Option<String>,
    pub collateral_type: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub custom_id: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query parameters for listing collateral
#[derive(Debug, Deserialize, IntoParams)]
pub struct CollateralListQueryParams {
    #[param(example = 20)]
    pub limit: Option<i64>,

    #[param(example = 0)]
    pub offset: Option<i64>,

    #[param(example = "real_estate")]
    pub collateral_type: Option<String>,

    #[param(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub created_by: Option<String>,
}

// ============================================================================
// CONVERSION FUNCTIONS
// ============================================================================

fn collateral_to_response(collateral: Collateral) -> CollateralResponse {
    CollateralResponse {
        id: collateral.id.to_string(),
        name: collateral.name,
        collateral_type: collateral.collateral_type,
        description: collateral.description,
        address: collateral.address,
        custom_id: collateral.custom_id,
        double_pledged: collateral.double_pledged.map(|u| u.to_string()),
        created_at: collateral.created_at.to_rfc3339(),
        updated_at: collateral.updated_at.to_rfc3339(),
        metadata: collateral.metadata,
    }
}

fn collateral_with_duplicates_to_response(
    data: CollateralWithDuplicates,
) -> CollateralWithDuplicatesResponse {
    CollateralWithDuplicatesResponse {
        collateral: collateral_to_response(data.collateral),
        potential_duplicates: data
            .potential_duplicates
            .into_iter()
            .map(collateral_to_response)
            .collect(),
    }
}

// ============================================================================
// API HANDLERS
// ============================================================================

/// Create a new collateral record
///
/// Creates a new collateral asset and initiates async duplicate detection.
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/collateral",
    request_body = CreateCollateralApiRequest,
    responses(
        (status = 201, description = "Collateral created successfully", body = CollateralResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state, headers))]
pub async fn create_collateral_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateCollateralApiRequest>,
) -> Result<(StatusCode, Json<CollateralResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!("Creating collateral: {}", req.name);

    let user = extract_current_user(&headers)
        .await
        .map_err(|e| internal_error(&format!("Failed to extract user: {}", e)))?;

    let db_req = DbCreateRequest {
        name: req.name,
        collateral_type: req.collateral_type,
        description: req.description,
        address: req.address,
        custom_id: req.custom_id,
        metadata: req.metadata,
        created_by: Some(user.user_id),
    };

    let collateral = create_collateral(&state.db_pool, db_req)
        .await
        .map_err(|e| internal_error(&format!("Failed to create collateral: {}", e)))?;

    info!("Collateral created successfully: {}", collateral.id);
    Ok((StatusCode::CREATED, Json(collateral_to_response(collateral))))
}

/// Get collateral by ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id)?;

    let collateral = get_collateral_by_id(&state.db_pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error(&format!("Collateral {}", id)),
            _ => internal_error(&format!("Failed to get collateral: {}", e)),
        })?;

    Ok(Json(collateral_to_response(collateral)))
}

/// Get collateral with potential duplicates
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/collateral/{id}/duplicates",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral found with duplicates", body = CollateralWithDuplicatesResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_with_duplicates_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CollateralWithDuplicatesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id)?;

    let data = get_collateral_with_duplicates(&state.db_pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error(&format!("Collateral {}", id)),
            _ => internal_error(&format!("Failed to get collateral: {}", e)),
        })?;

    Ok(Json(collateral_with_duplicates_to_response(data)))
}

/// Get collateral by custom ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/collateral/custom/{custom_id}",
    params(
        ("custom_id" = String, Path, description = "Custom collateral ID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_by_custom_id_handler(
    State(state): State<ApiState>,
    Path(custom_id): Path<String>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collateral = get_collateral_by_custom_id(&state.db_pool, &custom_id)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => {
                not_found_error(&format!("Collateral with custom_id {}", custom_id))
            }
            _ => internal_error(&format!("Failed to get collateral: {}", e)),
        })?;

    Ok(Json(collateral_to_response(collateral)))
}

/// Get collateral by deal ID
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/collateral/deal/{deal_id}",
    params(
        ("deal_id" = String, Path, description = "Deal UUID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_by_deal_id_handler(
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,
) -> Result<Json<Option<CollateralResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&deal_id)?;

    let collateral = get_collateral_by_deal_id(&state.db_pool, uuid)
        .await
        .map_err(|e| internal_error(&format!("Failed to get collateral: {}", e)))?;

    Ok(Json(collateral.map(collateral_to_response)))
}

/// List collateral with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/taxonomy/collateral",
    params(CollateralListQueryParams),
    responses(
        (status = 200, description = "List of collateral", body = Vec<CollateralResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn list_collateral_handler(
    State(state): State<ApiState>,
    Query(params): Query<CollateralListQueryParams>,
) -> Result<Json<Vec<CollateralResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let created_by = if let Some(id_str) = params.created_by {
        Some(parse_uuid(&id_str)?)
    } else {
        None
    };

    let collateral_list = list_collateral(
        &state.db_pool,
        params.collateral_type,
        created_by,
        params.limit.unwrap_or(20),
        params.offset.unwrap_or(0),
    )
    .await
    .map_err(|e| internal_error(&format!("Failed to list collateral: {}", e)))?;

    let response = collateral_list.into_iter().map(collateral_to_response).collect();

    Ok(Json(response))
}

/// Update collateral
#[utoipa::path(
    put,
    path = "/api/v1/taxonomy/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    request_body = UpdateCollateralApiRequest,
    responses(
        (status = 200, description = "Collateral updated successfully", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn update_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCollateralApiRequest>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id)?;

    let db_req = DbUpdateRequest {
        name: req.name,
        collateral_type: req.collateral_type,
        description: req.description,
        address: req.address,
        custom_id: req.custom_id,
        metadata: req.metadata,
    };

    let collateral = update_collateral(&state.db_pool, uuid, db_req)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error(&format!("Collateral {}", id)),
            _ => internal_error(&format!("Failed to update collateral: {}", e)),
        })?;

    Ok(Json(collateral_to_response(collateral)))
}

/// Delete collateral
#[utoipa::path(
    delete,
    path = "/api/v1/taxonomy/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral deleted successfully", body = DeleteResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn delete_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id)?;

    delete_collateral(&state.db_pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error(&format!("Collateral {}", id)),
            _ => internal_error(&format!("Failed to delete collateral: {}", e)),
        })?;

    Ok(Json(DeleteResponse {
        success: true,
        message: format!("Collateral {} deleted successfully", id),
    }))
}

/// Create collateral batch
#[utoipa::path(
    post,
    path = "/api/v1/taxonomy/collateral/batch",
    request_body = Vec<CreateCollateralApiRequest>,
    responses(
        (status = 201, description = "Batch created successfully", body = BatchOperationResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Taxonomy - Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state, headers))]
pub async fn create_collateral_batch_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(requests): Json<Vec<CreateCollateralApiRequest>>,
) -> Result<(StatusCode, Json<BatchOperationResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!("Creating collateral batch with {} items", requests.len());

    let user = extract_current_user(&headers)
        .await
        .map_err(|e| internal_error(&format!("Failed to extract user: {}", e)))?;

    let db_requests: Vec<DbCreateRequest> = requests
        .into_iter()
        .map(|req| DbCreateRequest {
            name: req.name,
            collateral_type: req.collateral_type,
            description: req.description,
            address: req.address,
            custom_id: req.custom_id,
            metadata: req.metadata,
            created_by: Some(user.user_id),
        })
        .collect();

    let results = create_collateral_batch(&state.db_pool, db_requests)
        .await
        .map_err(|e| internal_error(&format!("Failed to create collateral batch: {}", e)))?;

    let response = BatchOperationResponse {
        success: true,
        created_count: results.len(),
        failed_count: 0,
        errors: vec![],
    };

    info!("Batch creation successful: {} items created", results.len());
    Ok((StatusCode::CREATED, Json(response)))
}
