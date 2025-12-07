//! Collateral API Handlers
//!
//! Provides REST API endpoints for collateral CRUD operations with duplicate detection.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use utoipa::{IntoParams, ToSchema};

use arda_credit_db::{
    create_collateral, create_collateral_batch, delete_collateral, get_collateral_by_custom_id,
    get_collateral_by_deal_id, get_collateral_by_id, list_collateral, update_collateral,
    Collateral, CreateCollateralRequest as DbCreateRequest,
    UpdateCollateralRequest as DbUpdateRequest,
};

use crate::config::types::{ApiState, ErrorResponse};
use crate::handlers::taxonomy::common::{
    internal_error, not_found_error, parse_uuid, BatchOperationResponse, DeleteResponse,
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
    "double_pledged": null,
    "intentional_double_pledge": false,
    "double_pledge_notes": null,
    "deal_count": 2,
    "deal_ids": ["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"]
}))]
pub struct CollateralResponse {
    pub id: String,
    pub name: String,
    pub collateral_type: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub custom_id: Option<String>,
    pub double_pledged: Option<String>,
    pub has_potential_duplicates: Option<bool>,
    pub intentional_double_pledge: Option<bool>,
    pub double_pledge_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_ids: Option<Vec<String>>,
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
    pub intentional_double_pledge: Option<bool>,
    pub double_pledge_notes: Option<String>,
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
        has_potential_duplicates: collateral.has_potential_duplicates,
        intentional_double_pledge: collateral.intentional_double_pledge,
        double_pledge_notes: collateral.double_pledge_notes,
        created_at: collateral.created_at.to_rfc3339(),
        updated_at: collateral.updated_at.to_rfc3339(),
        metadata: collateral.metadata,
        deal_count: None,
        deal_ids: None,
    }
}

fn collateral_with_deals_to_response(
    collateral: Collateral,
    deal_ids: Vec<uuid::Uuid>,
) -> CollateralResponse {
    let deal_count = deal_ids.len() as i64;
    CollateralResponse {
        id: collateral.id.to_string(),
        name: collateral.name,
        collateral_type: collateral.collateral_type,
        description: collateral.description,
        address: collateral.address,
        custom_id: collateral.custom_id,
        double_pledged: collateral.double_pledged.map(|u| u.to_string()),
        has_potential_duplicates: collateral.has_potential_duplicates,
        intentional_double_pledge: collateral.intentional_double_pledge,
        double_pledge_notes: collateral.double_pledge_notes,
        created_at: collateral.created_at.to_rfc3339(),
        updated_at: collateral.updated_at.to_rfc3339(),
        metadata: collateral.metadata,
        deal_count: Some(deal_count),
        deal_ids: Some(deal_ids.iter().map(|id| id.to_string()).collect()),
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
    path = "/api/v1/collateral",
    request_body = CreateCollateralApiRequest,
    responses(
        (status = 201, description = "Collateral created successfully", body = CollateralResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state, headers))]
pub async fn create_collateral_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateCollateralApiRequest>,
) -> Result<(StatusCode, Json<CollateralResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!("Creating collateral: {}", req.name);

    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;

    let db_req = DbCreateRequest {
        name: req.name,
        collateral_type: req.collateral_type,
        description: req.description,
        address: req.address,
        custom_id: req.custom_id,
        metadata: req.metadata,
        created_by: Some(user.id),
    };

    let collateral = create_collateral(&state.pool, db_req)
        .await
        .map_err(|e| internal_error("create_collateral", e))?;

    info!("Collateral created successfully: {}", collateral.id);

    // Publish CollateralCreated event (fire-and-forget)
    if let Some(publisher) = &state.event_publisher {
        let event = crate::events::CollateralEvent::CollateralCreated {
            event_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            collateral: crate::events::CollateralPayload {
                id: collateral.id,
                name: collateral.name.clone(),
                collateral_type: collateral.collateral_type.clone().unwrap_or_default(),
                address: collateral.address.clone(),
                custom_id: collateral.custom_id.clone(),
                description: collateral.description.clone(),
            },
        };
        let publisher = publisher.clone();
        tokio::spawn(async move {
            if let Err(e) = publisher.publish_collateral(event).await {
                tracing::warn!("Failed to publish CollateralCreated event: {}", e);
            }
        });
    }

    Ok((
        StatusCode::CREATED,
        Json(collateral_to_response(collateral)),
    ))
}

/// Get collateral by ID
#[utoipa::path(
    get,
    path = "/api/v1/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    let collateral = get_collateral_by_id(&state.pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("get_collateral", e),
        })?;

    Ok(Json(collateral_to_response(collateral)))
}

/// Get collateral by ID (internal endpoint - no auth required)
///
/// This endpoint is for service-to-service communication with arda-collateral-intel.
/// It returns the same data as get_collateral_handler but bypasses authentication.
#[utoipa::path(
    get,
    path = "/api/v1/internal/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Internal",
    security()
)]
#[instrument(skip(state))]
pub async fn get_collateral_internal_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    let collateral = get_collateral_by_id(&state.pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("get_collateral_internal", e),
        })?;

    Ok(Json(collateral_to_response(collateral)))
}

/// Get collateral by custom ID
#[utoipa::path(
    get,
    path = "/api/v1/collateral/custom/{custom_id}",
    params(
        ("custom_id" = String, Path, description = "Custom collateral ID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_by_custom_id_handler(
    State(state): State<ApiState>,
    Path(custom_id): Path<String>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collateral = get_collateral_by_custom_id(&state.pool, &custom_id)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &custom_id),
            _ => internal_error("get_collateral", e),
        })?;

    Ok(Json(collateral_to_response(collateral)))
}

/// Get collateral by deal ID
#[utoipa::path(
    get,
    path = "/api/v1/collateral/deal/{deal_id}",
    params(
        ("deal_id" = String, Path, description = "Deal UUID")
    ),
    responses(
        (status = 200, description = "Collateral found", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_by_deal_id_handler(
    State(state): State<ApiState>,
    Path(deal_id): Path<String>,
) -> Result<Json<Option<CollateralResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&deal_id, "deal_id")?;

    let collateral = get_collateral_by_deal_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("get_collateral_by_deal_id", e))?;

    Ok(Json(collateral.map(collateral_to_response)))
}

/// List collateral with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/collateral",
    params(CollateralListQueryParams),
    responses(
        (status = 200, description = "List of collateral", body = Vec<CollateralResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn list_collateral_handler(
    State(state): State<ApiState>,
    Query(params): Query<CollateralListQueryParams>,
) -> Result<Json<Vec<CollateralResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let created_by = if let Some(id_str) = params.created_by {
        Some(parse_uuid(&id_str, "created_by")?)
    } else {
        None
    };

    let collateral_list = list_collateral(
        &state.pool,
        params.collateral_type,
        created_by,
        params.limit.unwrap_or(20),
        params.offset.unwrap_or(0),
    )
    .await
    .map_err(|e| internal_error("list_collateral", e))?;

    let response = collateral_list
        .into_iter()
        .map(collateral_to_response)
        .collect();

    Ok(Json(response))
}

/// Update collateral
#[utoipa::path(
    put,
    path = "/api/v1/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    request_body = UpdateCollateralApiRequest,
    responses(
        (status = 200, description = "Collateral updated successfully", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn update_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCollateralApiRequest>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    // Fetch old collateral state for change detection
    let old_collateral = get_collateral_by_id(&state.pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("get_collateral_by_id", e),
        })?;

    let db_req = DbUpdateRequest {
        name: req.name.clone(),
        collateral_type: req.collateral_type.clone(),
        description: req.description.clone(),
        address: req.address.clone(),
        custom_id: req.custom_id.clone(),
        metadata: req.metadata.clone(),
        intentional_double_pledge: req.intentional_double_pledge,
        double_pledge_notes: req.double_pledge_notes.clone(),
    };

    let collateral = update_collateral(&state.pool, uuid, db_req)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("update_collateral", e),
        })?;

    // Publish CollateralUpdated event (fire-and-forget)
    if let Some(publisher) = &state.event_publisher {
        // Detect changes
        let mut changes = Vec::new();

        if let Some(ref new_name) = req.name {
            if *new_name != old_collateral.name {
                changes.push(crate::events::FieldChange {
                    field_name: "name".to_string(),
                    old_value: Some(old_collateral.name.clone()),
                    new_value: Some(new_name.clone()),
                });
            }
        }

        if let Some(ref new_type) = req.collateral_type {
            if Some(new_type.clone()) != old_collateral.collateral_type {
                changes.push(crate::events::FieldChange {
                    field_name: "collateral_type".to_string(),
                    old_value: old_collateral.collateral_type.clone(),
                    new_value: Some(new_type.clone()),
                });
            }
        }

        if let Some(ref new_desc) = req.description {
            if Some(new_desc.clone()) != old_collateral.description {
                changes.push(crate::events::FieldChange {
                    field_name: "description".to_string(),
                    old_value: old_collateral.description.clone(),
                    new_value: Some(new_desc.clone()),
                });
            }
        }

        if let Some(ref new_addr) = req.address {
            if Some(new_addr.clone()) != old_collateral.address {
                changes.push(crate::events::FieldChange {
                    field_name: "address".to_string(),
                    old_value: old_collateral.address.clone(),
                    new_value: Some(new_addr.clone()),
                });
            }
        }

        if let Some(ref new_custom_id) = req.custom_id {
            if Some(new_custom_id.clone()) != old_collateral.custom_id {
                changes.push(crate::events::FieldChange {
                    field_name: "custom_id".to_string(),
                    old_value: old_collateral.custom_id.clone(),
                    new_value: Some(new_custom_id.clone()),
                });
            }
        }

        if !changes.is_empty() {
            let event = crate::events::CollateralEvent::CollateralUpdated {
                event_id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                collateral: crate::events::CollateralPayload {
                    id: collateral.id,
                    name: collateral.name.clone(),
                    collateral_type: collateral.collateral_type.clone().unwrap_or_default(),
                    address: collateral.address.clone(),
                    custom_id: collateral.custom_id.clone(),
                    description: collateral.description.clone(),
                },
                changes,
            };
            let publisher = publisher.clone();
            tokio::spawn(async move {
                if let Err(e) = publisher.publish_collateral(event).await {
                    tracing::warn!("Failed to publish CollateralUpdated event: {}", e);
                }
            });
        }
    }

    Ok(Json(collateral_to_response(collateral)))
}

/// Delete collateral
#[utoipa::path(
    delete,
    path = "/api/v1/collateral/{id}",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral deleted successfully", body = DeleteResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn delete_collateral_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    delete_collateral(&state.pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("delete_collateral", e),
        })?;

    // Publish CollateralDeleted event (fire-and-forget)
    if let Some(publisher) = &state.event_publisher {
        let event = crate::events::CollateralEvent::CollateralDeleted {
            event_id: uuid::Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            collateral_id: uuid,
        };
        let publisher = publisher.clone();
        tokio::spawn(async move {
            if let Err(e) = publisher.publish_collateral(event).await {
                tracing::warn!("Failed to publish CollateralDeleted event: {}", e);
            }
        });
    }

    Ok(Json(DeleteResponse {
        success: true,
        message: format!("Collateral {} deleted successfully", id),
        id: id.clone(),
    }))
}

/// Create collateral batch
#[utoipa::path(
    post,
    path = "/api/v1/collateral/batch",
    request_body = Vec<CreateCollateralApiRequest>,
    responses(
        (status = 201, description = "Batch created successfully", body = BatchOperationResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state, headers))]
pub async fn create_collateral_batch_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(requests): Json<Vec<CreateCollateralApiRequest>>,
) -> Result<(StatusCode, Json<BatchOperationResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!("Creating collateral batch with {} items", requests.len());

    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;

    let db_requests: Vec<DbCreateRequest> = requests
        .into_iter()
        .map(|req| DbCreateRequest {
            name: req.name,
            collateral_type: req.collateral_type,
            description: req.description,
            address: req.address,
            custom_id: req.custom_id,
            metadata: req.metadata,
            created_by: Some(user.id),
        })
        .collect();

    let results = create_collateral_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("create_collateral_batch", e))?;

    let successful_ids: Vec<String> = results.iter().map(|c| c.id.to_string()).collect();

    // Publish CollateralCreated events for each item in batch (fire-and-forget)
    if let Some(publisher) = &state.event_publisher {
        for collateral in &results {
            let event = crate::events::CollateralEvent::CollateralCreated {
                event_id: uuid::Uuid::new_v4(),
                timestamp: chrono::Utc::now(),
                collateral: crate::events::CollateralPayload {
                    id: collateral.id,
                    name: collateral.name.clone(),
                    collateral_type: collateral.collateral_type.clone().unwrap_or_default(),
                    address: collateral.address.clone(),
                    custom_id: collateral.custom_id.clone(),
                    description: collateral.description.clone(),
                },
            };
            let publisher = publisher.clone();
            tokio::spawn(async move {
                if let Err(e) = publisher.publish_collateral(event).await {
                    tracing::warn!("Failed to publish CollateralCreated event in batch: {}", e);
                }
            });
        }
    }

    let response = BatchOperationResponse {
        success_count: results.len(),
        error_count: 0,
        total: results.len(),
        successful_ids,
        errors: vec![],
    };

    info!("Batch creation successful: {} items created", results.len());
    Ok((StatusCode::CREATED, Json(response)))
}

/// Get all deals that share a specific collateral
///
/// Returns all deal IDs that reference the given collateral ID, useful for identifying
/// self double-pledge scenarios.
#[utoipa::path(
    get,
    path = "/api/v1/collateral/{id}/deals",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "List of deal IDs", body = Vec<String>),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_deals_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    // First check if collateral exists
    let _collateral = arda_credit_db::get_collateral_by_id(&state.pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("get_collateral_by_id", e),
        })?;

    let deal_ids = arda_credit_db::get_deals_by_collateral_id(&state.pool, uuid)
        .await
        .map_err(|e| internal_error("get_deals_by_collateral_id", e))?;

    Ok(Json(deal_ids.iter().map(|id| id.to_string()).collect()))
}

/// Get collateral with deal count
///
/// Returns collateral details along with the count of deals that reference it and their IDs.
#[utoipa::path(
    get,
    path = "/api/v1/collateral/{id}/with-deals",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    responses(
        (status = 200, description = "Collateral with deal count", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn get_collateral_with_deals_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    let collateral_with_count = arda_credit_db::get_collateral_with_deal_count(&state.pool, uuid)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("get_collateral_with_deal_count", e),
        })?;

    Ok(Json(collateral_with_deals_to_response(
        collateral_with_count.collateral,
        collateral_with_count.deal_ids,
    )))
}

/// Request to mark collateral as intentionally double-pledged
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MarkIntentionalDoublePledgeRequest {
    #[schema(example = true)]
    pub intentional: bool,

    #[schema(example = "This collateral is intentionally used for multiple tranches")]
    pub notes: Option<String>,
}

/// Mark collateral as intentionally double-pledged
///
/// Allows users to explicitly mark that a collateral is intentionally being used
/// across multiple deals. This distinguishes legitimate multi-deal collateral from
/// unintentional duplicate detection.
#[utoipa::path(
    post,
    path = "/api/v1/collateral/{id}/mark-intentional-double-pledge",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    request_body = MarkIntentionalDoublePledgeRequest,
    responses(
        (status = 200, description = "Collateral updated successfully", body = CollateralResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn mark_intentional_double_pledge_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<MarkIntentionalDoublePledgeRequest>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    let collateral =
        arda_credit_db::mark_intentional_double_pledge(&state.pool, uuid, req.intentional, req.notes)
            .await
            .map_err(|e| match e {
                arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
                _ => internal_error("mark_intentional_double_pledge", e),
            })?;

    info!(
        "Collateral {} marked as intentional_double_pledge={}",
        uuid, req.intentional
    );

    Ok(Json(collateral_to_response(collateral)))
}

/// List collateral used in multiple deals
///
/// Returns collateral that is referenced by 2 or more deals, useful for identifying
/// self double-pledge scenarios.
#[utoipa::path(
    get,
    path = "/api/v1/collateral/multi-deal",
    params(
        ("limit" = Option<i64>, Query, description = "Maximum number of results (default: 20)"),
        ("offset" = Option<i64>, Query, description = "Offset for pagination (default: 0)")
    ),
    responses(
        (status = 200, description = "List of collateral with multiple deals", body = Vec<CollateralResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state))]
pub async fn list_multi_deal_collateral_handler(
    State(state): State<ApiState>,
    Query(params): Query<CollateralListQueryParams>,
) -> Result<Json<Vec<CollateralResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let results = arda_credit_db::list_multi_deal_collateral(
        &state.pool,
        params.limit.unwrap_or(20),
        params.offset.unwrap_or(0),
    )
    .await
    .map_err(|e| internal_error("list_multi_deal_collateral", e))?;

    let response = results
        .into_iter()
        .map(|r| collateral_with_deals_to_response(r.collateral, r.deal_ids))
        .collect();

    Ok(Json(response))
}
