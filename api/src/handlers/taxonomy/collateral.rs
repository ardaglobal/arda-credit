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
use uuid::Uuid;

use arda_credit_db::{
    create_collateral, create_collateral_batch, delete_collateral, get_collateral_by_custom_id,
    get_collateral_by_deal_id, get_collateral_by_id, list_collateral, update_collateral,
    Collateral, CreateCollateralRequest as DbCreateRequest,
    UpdateCollateralRequest as DbUpdateRequest,
};
use arda_credit_lib::TransactionType;
use rust_decimal::Decimal;

use crate::config::types::{ApiState, ErrorResponse};
use crate::handlers::taxonomy::common::{
    internal_error, not_found_error, parse_uuid, BatchOperationResponse, DeleteResponse,
};
use crate::middleware::session_middleware::extract_current_user;

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Collateral API response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "123 Main Street Property",
    "collateral_type": "real_estate",
    "description": "Commercial property in downtown",
    "address": "123 Main St, New York, NY 10001",
    "custom_id": "PROP-2025-001",
    "double_pledged": null,
    "deal_count": 2,
    "deal_ids": ["550e8400-e29b-41d4-a716-446655440001", "550e8400-e29b-41d4-a716-446655440002"],
    "is_double_pledged": true,
    "has_detection": true,
    "detection_count": 1,
    "last_detection_at": "2025-12-09T10:30:00Z",
    "detection_status": "potential_duplicates"
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
    pub created_at: String,
    pub updated_at: String,
    pub metadata: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deal_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_double_pledged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_detection: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_detection_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection_status: Option<String>,
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

/// Request to associate collateral with deal
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssociateCollateralRequest {
    pub deal_id: String,
}

/// Response from association
#[derive(Debug, Serialize, ToSchema)]
pub struct AssociationResponse {
    pub collateral_id: String,
    pub deal_id: String,
    pub transaction_id: String,
    pub associated_at: chrono::DateTime<chrono::Utc>,
}

/// Request to disassociate collateral from deal
#[derive(Debug, Deserialize, ToSchema)]
pub struct DisassociateCollateralRequest {
    pub deal_id: String,
}

/// Response from disassociation
#[derive(Debug, Serialize, ToSchema)]
pub struct DisassociationResponse {
    pub collateral_id: String,
    pub deal_id: String,
    pub transaction_id: String,
    pub disassociated_at: chrono::DateTime<chrono::Utc>,
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
        created_at: collateral.created_at.to_rfc3339(),
        updated_at: collateral.updated_at.to_rfc3339(),
        metadata: collateral.metadata,
        deal_count: None,
        deal_ids: None,
        is_double_pledged: None,
        has_detection: Some(collateral.has_detection),
        detection_count: Some(collateral.detection_count),
        last_detection_at: collateral.last_detection_at.map(|dt| dt.to_rfc3339()),
        detection_status: collateral.detection_status,
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

    // Record creation transaction for zkVM
    // Metadata must encode collateral_type in first 32 bytes for zkVM
    let metadata = Some(serde_json::json!({
        "collateral_type": collateral.collateral_type.clone().unwrap_or_default(),
        "name": collateral.name.clone(),
        "address": collateral.address.clone(),
        "custom_id": collateral.custom_id.clone(),
        "created_by": user.id.to_string(),
    }));

    let tx_result = state
        .transaction_recording
        .record_transaction_with_metadata(
            arda_credit_lib::TransactionType::CollateralCreation,
            user.id,
            None, // No deal_id yet (not associated)
            rust_decimal::Decimal::ZERO,
            metadata,
            None,                // originator_company_id
            None,                // investor_company_id
            Some(collateral.id), // collateral_id
        )
        .await;

    match tx_result {
        Ok(tx_id) => {
            info!(
                "Recorded CollateralCreation transaction {} for collateral {}",
                tx_id, collateral.id
            );
        }
        Err(e) => {
            // Decision: Fail request or log and continue?
            tracing::error!("Failed to record collateral creation transaction: {}", e);
            // For MVP: Log but don't fail (collateral was created successfully)
            // For production: Consider failing for strict consistency
        }
    }

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
#[instrument(skip(state, headers))]
pub async fn update_collateral_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpdateCollateralApiRequest>,
) -> Result<Json<CollateralResponse>, (StatusCode, Json<ErrorResponse>)> {
    let uuid = parse_uuid(&id, "collateral_id")?;

    // Extract current user for audit trail
    let current_user = extract_current_user(&headers);
    let user_id = current_user
        .as_ref()
        .map(|u| u.id)
        .unwrap_or_else(|| Uuid::nil());

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
    };

    let collateral = update_collateral(&state.pool, uuid, db_req)
        .await
        .map_err(|e| match e {
            arda_credit_db::DbError::NotFound(_) => not_found_error("Collateral", &id),
            _ => internal_error("update_collateral", e),
        })?;

    // Detect changes for both transaction recording and event publishing
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

    // Record update transaction for audit trail if there are changes
    if !changes.is_empty() {
        // Record update transaction for zkVM
        let metadata = Some(serde_json::json!({
            "collateral_type": collateral.collateral_type.clone(),
            "name": collateral.name.clone(),
            "address": collateral.address.clone(),
            "updated_by": user_id.to_string(),
            "updated_fields": serde_json::json!({
                "name": req.name.is_some(),
                "collateral_type": req.collateral_type.is_some(),
                "description": req.description.is_some(),
                "address": req.address.is_some(),
                "custom_id": req.custom_id.is_some(),
                "metadata": req.metadata.is_some(),
            }),
        }));

        if let Err(e) = state
            .transaction_recording
            .record_transaction_with_metadata(
                TransactionType::CollateralUpdate,
                user_id,
                None, // No deal_id for update
                Decimal::ZERO,
                metadata,
                None,       // originator_company_id
                None,       // investor_company_id
                Some(uuid), // collateral_id
            )
            .await
        {
            tracing::error!("Failed to record collateral update transaction: {}", e);
            // Decision: Log and continue, or return error?
            // For MVP: Log and continue
        }
    }

    // Publish CollateralUpdated event (fire-and-forget)
    if let Some(publisher) = &state.event_publisher {
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
    security()
)]
#[instrument(skip(state, headers))]
pub async fn create_collateral_batch_handler(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(requests): Json<Vec<CreateCollateralApiRequest>>,
) -> Result<(StatusCode, Json<BatchOperationResponse>), (StatusCode, Json<ErrorResponse>)> {
    info!("Creating collateral batch with {} items", requests.len());

    let current_user = extract_current_user(&headers);
    let user_id = current_user.as_ref().map(|u| u.id);

    let db_requests: Vec<DbCreateRequest> = requests
        .into_iter()
        .map(|req| DbCreateRequest {
            name: req.name,
            collateral_type: req.collateral_type,
            description: req.description,
            address: req.address,
            custom_id: req.custom_id,
            metadata: req.metadata,
            created_by: user_id,
        })
        .collect();

    let results = create_collateral_batch(&state.pool, db_requests)
        .await
        .map_err(|e| internal_error("create_collateral_batch", e))?;

    let successful_ids: Vec<String> = results.iter().map(|c| c.id.to_string()).collect();

    // Record transactions and publish events (only if user is authenticated)
    if let Some(user) = current_user.as_ref() {
        // Record CollateralCreation transactions for zkVM
        for collateral in &results {
            let metadata = Some(serde_json::json!({
                "collateral_type": collateral.collateral_type.clone().unwrap_or_default(),
                "name": collateral.name.clone(),
                "address": collateral.address.clone(),
                "custom_id": collateral.custom_id.clone(),
                "created_by": user.id.to_string(),
            }));

            let tx_result = state
                .transaction_recording
                .record_transaction_with_metadata(
                    arda_credit_lib::TransactionType::CollateralCreation,
                    user.id,
                    None, // No deal_id yet (not associated)
                    rust_decimal::Decimal::ZERO,
                    metadata,
                    None,                // originator_company_id
                    None,                // investor_company_id
                    Some(collateral.id), // collateral_id
                )
                .await;

            match tx_result {
                Ok(tx_id) => {
                    info!(
                        "Recorded CollateralCreation transaction {} for collateral {}",
                        tx_id, collateral.id
                    );
                }
                Err(e) => {
                    tracing::error!("Failed to record collateral creation transaction: {}", e);
                }
            }
        }

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
    } else {
        info!("Skipping transaction recording and event publishing for anonymous collateral batch creation");
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

/// Associate collateral with a deal
///
/// Links an existing piece of collateral to an existing deal.
/// This will be validated in the zkVM to prevent double-pledging.
///
#[utoipa::path(
    post,
    path = "/api/v1/collateral/{collateral_id}/associate",
    request_body = AssociateCollateralRequest,
    responses(
        (status = 200, description = "Collateral associated successfully", body = AssociationResponse),
        (status = 400, description = "Invalid request or double-pledge detected", body = ErrorResponse),
        (status = 404, description = "Collateral or deal not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state, headers))]
pub async fn associate_collateral_handler(
    State(state): State<ApiState>,
    Path(collateral_id_str): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AssociateCollateralRequest>,
) -> Result<Json<AssociationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collateral_id = parse_uuid(&collateral_id_str, "collateral_id")?;
    let deal_id = parse_uuid(&req.deal_id, "deal_id")?;

    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;

    // Validate collateral exists
    let _collateral = get_collateral_by_id(&state.pool, collateral_id)
        .await
        .map_err(|_| not_found_error("Collateral", &collateral_id_str))?;

    // Validate deal exists and get company IDs
    let deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| not_found_error("Deal", &req.deal_id))?;

    // Check for existing active associations (double-pledge check)
    let existing_deal = sqlx::query!(
        r#"
        SELECT id FROM deals
        WHERE collateral_id = $1
          AND deal_status IN ('originated', 'active', 'paid_off')
        LIMIT 1
        "#,
        collateral_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| internal_error("check_existing_association", e))?;

    if let Some(existing) = existing_deal {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "CollateralAlreadyPledged".to_string(),
                message: format!("Collateral is already pledged to deal {}", existing.id),
                details: None,
            }),
        ));
    }

    // Update deal to include collateral_id
    sqlx::query!(
        "UPDATE deals SET collateral_id = $1, updated_at = NOW() WHERE id = $2",
        collateral_id,
        deal_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| internal_error("update_deal_collateral", e))?;

    // Record association transaction for zkVM
    // Metadata format: bytes 0-31 = collateral_id, bytes 32-63 = deal_id
    let assoc_metadata = Some(serde_json::json!({
        "collateral_id": collateral_id.to_string(),
        "deal_id": deal_id.to_string(),
        "associated_by": user.id.to_string(),
    }));

    let tx_id = state
        .transaction_recording
        .record_transaction_with_metadata(
            TransactionType::CollateralDealAssociation,
            user.id,
            Some(deal_id),
            Decimal::ZERO,
            assoc_metadata,
            deal.as_ref().map(|d| d.originator_id), // originator_company_id
            deal.as_ref().and_then(|d| d.investor_id), // investor_company_id
            Some(collateral_id),                    // collateral_id
        )
        .await
        .map_err(|e| internal_error("record_association_transaction", e))?;

    info!(
        "Associated collateral {} with deal {}, transaction {}",
        collateral_id, deal_id, tx_id
    );

    Ok(Json(AssociationResponse {
        collateral_id: collateral_id.to_string(),
        deal_id: deal_id.to_string(),
        transaction_id: tx_id.to_string(),
        associated_at: chrono::Utc::now(),
    }))
}

/// Disassociate collateral from a deal
///
/// Removes the link between collateral and a deal.
/// Typically used when a deal is closed or when replacing collateral.
///
#[utoipa::path(
    delete,
    path = "/api/v1/collateral/{collateral_id}/associate",
    params(
        ("collateral_id" = String, Path, description = "Collateral UUID")
    ),
    request_body = DisassociateCollateralRequest,
    responses(
        (status = 200, description = "Collateral disassociated successfully", body = DisassociationResponse),
        (status = 404, description = "Association not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Collateral",
    security(("bearer" = []))
)]
#[instrument(skip(state, headers))]
pub async fn disassociate_collateral_handler(
    State(state): State<ApiState>,
    Path(collateral_id_str): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<DisassociateCollateralRequest>,
) -> Result<Json<DisassociationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collateral_id = parse_uuid(&collateral_id_str, "collateral_id")?;
    let deal_id = parse_uuid(&req.deal_id, "deal_id")?;

    let current_user = extract_current_user(&headers);
    let user = current_user
        .as_ref()
        .ok_or_else(|| internal_error("extract_current_user", "User not authenticated"))?;

    // Get deal to extract company IDs before removing collateral
    let deal = arda_credit_db::get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| not_found_error("Deal", &req.deal_id))?;

    // Remove collateral_id from deal
    sqlx::query!(
        "UPDATE deals SET collateral_id = NULL, updated_at = NOW() WHERE id = $1 AND collateral_id = $2",
        deal_id,
        collateral_id,
    )
    .execute(&state.pool)
    .await
    .map_err(|e| internal_error("update_deal_collateral", e))?;

    // Record disassociation transaction for zkVM
    // Metadata format: bytes 0-31 = collateral_id, bytes 32-63 = deal_id
    let disassoc_metadata = Some(serde_json::json!({
        "collateral_id": collateral_id.to_string(),
        "deal_id": deal_id.to_string(),
        "disassociated_by": user.id.to_string(),
    }));

    let tx_id = state
        .transaction_recording
        .record_transaction_with_metadata(
            TransactionType::CollateralDealDisassociation,
            user.id,
            Some(deal_id),
            Decimal::ZERO,
            disassoc_metadata,
            deal.as_ref().map(|d| d.originator_id), // originator_company_id
            deal.as_ref().and_then(|d| d.investor_id), // investor_company_id
            Some(collateral_id),                    // collateral_id
        )
        .await
        .map_err(|e| internal_error("record_disassociation_transaction", e))?;

    info!(
        "Disassociated collateral {} from deal {}, transaction {}",
        collateral_id, deal_id, tx_id
    );

    Ok(Json(DisassociationResponse {
        collateral_id: collateral_id.to_string(),
        deal_id: deal_id.to_string(),
        transaction_id: tx_id.to_string(),
        disassociated_at: chrono::Utc::now(),
    }))
}

// ============================================================================
// NOTE: Audit trail functionality has been consolidated into /api/v1/state/transactions
// Use GET /api/v1/state/transactions?collateral_id={id} to retrieve collateral transaction history
// ============================================================================
