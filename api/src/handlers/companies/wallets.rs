///! Company wallet address API handlers
///!
///! This module provides REST API endpoints for managing company wallet addresses
///! with permission-based access control.
use crate::{api::router::ApiState, dto::wallet_dto::*};
use arda_credit_db::business::wallet::{
    self, CreateWalletAddressRequest as DbCreateRequest,
    GrantWalletPermissionRequest as DbGrantRequest, UpdateWalletAddressRequest as DbUpdateRequest,
    WalletAddressStatus, WalletAddressType, WalletPermissionType,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::{error, info};
use uuid::Uuid;

/// Query parameters for listing wallet addresses
#[derive(Debug, Deserialize, utoipa::IntoParams, utoipa::ToSchema)]
pub struct ListWalletsQuery {
    /// Filter by status
    #[param(example = "active")]
    pub status: Option<String>,
}

/// Create a new wallet address for a company
#[utoipa::path(
    post,
    path = "/api/v1/companies/{company_id}/wallets",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    request_body = CreateWalletAddressRequest,
    responses(
        (status = 201, description = "Wallet address created successfully", body = WalletAddressResponse),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn create_wallet_address(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    Json(request): Json<CreateWalletAddressRequest>,
) -> impl IntoResponse {
    // TODO: Extract claims from request when auth is properly implemented
    let created_by = Uuid::parse_str("2e918cf4-3d02-4eba-970c-a9f2630ecbde").expect("Valid UUID"); // chad@arda.xyz

    // Parse address_type
    let address_type: WalletAddressType = match request.address_type.parse() {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(format!("Invalid address_type: {}", e)),
            )
                .into_response()
        }
    };

    let db_request = DbCreateRequest {
        company_id,
        wallet_address: request.wallet_address,
        address_label: request.address_label,
        address_type,
        is_primary: request.is_primary,
        notes: request.notes,
        metadata: request.metadata,
    };

    match wallet::create_wallet_address(&state.pool, &db_request, created_by).await {
        Ok(wallet) => {
            info!(
                wallet_id = %wallet.id,
                company_id = %company_id,
                address = %wallet.wallet_address,
                "Wallet address created successfully"
            );
            (
                StatusCode::CREATED,
                Json(WalletAddressResponse::from(wallet)),
            )
                .into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to create wallet address");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to create wallet address"),
            )
                .into_response()
        }
    }
}

/// List wallet addresses for a company
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/wallets",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ListWalletsQuery
    ),
    responses(
        (status = 200, description = "Wallet addresses retrieved successfully", body = Vec<WalletAddressResponse>),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Company not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn list_wallet_addresses(
    Path(company_id): Path<Uuid>,
    Query(query): Query<ListWalletsQuery>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    let status: Option<WalletAddressStatus> = query.status.and_then(|s| s.parse().ok());

    match wallet::list_company_wallet_addresses(&state.pool, company_id, status).await {
        Ok(wallets) => {
            let response: Vec<WalletAddressResponse> =
                wallets.into_iter().map(|w| w.into()).collect();
            Json(response).into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to list wallet addresses");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to list wallet addresses"),
            )
                .into_response()
        }
    }
}

/// Get user's accessible wallet addresses in a company
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/users/{user_id}/wallets",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "User wallet addresses retrieved successfully", body = Vec<UserWalletAddressResponse>),
        (status = 403, description = "Access denied"),
        (status = 404, description = "User has no wallet access"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn get_user_wallet_addresses(
    Path((company_id, user_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    match wallet::get_user_wallet_addresses(&state.pool, user_id, company_id).await {
        Ok(wallets) => {
            let response: Vec<UserWalletAddressResponse> =
                wallets.into_iter().map(|w| w.into()).collect();
            Json(response).into_response()
        }
        Err(err) => {
            error!(error = %err, user_id = %user_id, company_id = %company_id, "Failed to get user wallet addresses");
            (StatusCode::NOT_FOUND, Json("User has no wallet access")).into_response()
        }
    }
}

/// Get a specific wallet address
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/wallets/{wallet_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("wallet_id" = Uuid, Path, description = "Wallet address ID")
    ),
    responses(
        (status = 200, description = "Wallet address retrieved successfully", body = WalletAddressResponse),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Wallet address not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn get_wallet_address(
    Path((company_id, wallet_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    match wallet::get_wallet_address_by_id(&state.pool, wallet_id).await {
        Ok(wallet) => {
            if wallet.company_id != company_id {
                return (
                    StatusCode::NOT_FOUND,
                    Json("Wallet address not found in this company"),
                )
                    .into_response();
            }
            Json(WalletAddressResponse::from(wallet)).into_response()
        }
        Err(err) => {
            error!(error = %err, wallet_id = %wallet_id, "Failed to get wallet address");
            (StatusCode::NOT_FOUND, Json("Wallet address not found")).into_response()
        }
    }
}

/// Update a wallet address
#[utoipa::path(
    put,
    path = "/api/v1/companies/{company_id}/wallets/{wallet_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("wallet_id" = Uuid, Path, description = "Wallet address ID")
    ),
    request_body = UpdateWalletAddressRequest,
    responses(
        (status = 200, description = "Wallet address updated successfully", body = WalletAddressResponse),
        (status = 400, description = "Validation error"),
        (status = 403, description = "Access denied - requires manage_address permission"),
        (status = 404, description = "Wallet address not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn update_wallet_address(
    Path((company_id, wallet_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
    Json(request): Json<UpdateWalletAddressRequest>,
) -> impl IntoResponse {
    // First verify wallet belongs to this company
    let existing = match wallet::get_wallet_address_by_id(&state.pool, wallet_id).await {
        Ok(w) => w,
        Err(_) => return (StatusCode::NOT_FOUND, Json("Wallet address not found")).into_response(),
    };

    if existing.company_id != company_id {
        return (
            StatusCode::NOT_FOUND,
            Json("Wallet address not found in this company"),
        )
            .into_response();
    }

    // Parse optional fields
    let address_type: Option<WalletAddressType> = request.address_type.and_then(|t| t.parse().ok());
    let status: Option<WalletAddressStatus> = request.status.and_then(|s| s.parse().ok());

    let db_request = DbUpdateRequest {
        address_label: request.address_label,
        address_type,
        status,
        is_primary: request.is_primary,
        notes: request.notes,
        metadata: request.metadata,
    };

    match wallet::update_wallet_address(&state.pool, wallet_id, &db_request).await {
        Ok(wallet) => {
            info!(wallet_id = %wallet_id, "Wallet address updated successfully");
            Json(WalletAddressResponse::from(wallet)).into_response()
        }
        Err(err) => {
            error!(error = %err, wallet_id = %wallet_id, "Failed to update wallet address");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to update wallet address"),
            )
                .into_response()
        }
    }
}

/// Grant wallet permission to a user
#[utoipa::path(
    post,
    path = "/api/v1/companies/{company_id}/wallets/permissions",
    params(
        ("company_id" = Uuid, Path, description = "Company ID")
    ),
    request_body = GrantWalletPermissionRequest,
    responses(
        (status = 201, description = "Permission granted successfully", body = WalletPermissionResponse),
        (status = 400, description = "Validation error - sensitive permissions require admin role"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Wallet or membership not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn grant_wallet_permission(
    Path(company_id): Path<Uuid>,
    State(state): State<ApiState>,
    Json(request): Json<GrantWalletPermissionRequest>,
) -> impl IntoResponse {
    // TODO: Extract claims from request when auth is properly implemented
    let granted_by = Uuid::parse_str("2e918cf4-3d02-4eba-970c-a9f2630ecbde").expect("Valid UUID"); // chad@arda.xyz

    // Parse permission_type
    let permission_type: WalletPermissionType = match request.permission_type.parse() {
        Ok(pt) => pt,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(format!("Invalid permission_type: {}", e)),
            )
                .into_response()
        }
    };

    // Parse daily_limit if provided
    let daily_limit: Option<Decimal> = request.daily_limit.and_then(|s| s.parse().ok());

    let db_request = DbGrantRequest {
        wallet_address_id: request.wallet_address_id,
        membership_id: request.membership_id,
        permission_type,
        daily_limit,
        requires_approval: request.requires_approval,
        expires_at: request.expires_at,
    };

    match wallet::grant_wallet_permission(&state.pool, &db_request, granted_by).await {
        Ok(permission) => {
            info!(
                permission_id = %permission.id,
                membership_id = %permission.membership_id,
                permission_type = %permission.permission_type,
                "Wallet permission granted successfully"
            );
            (
                StatusCode::CREATED,
                Json(WalletPermissionResponse::from(permission)),
            )
                .into_response()
        }
        Err(err) => {
            error!(error = %err, company_id = %company_id, "Failed to grant wallet permission");

            // Check if error is about admin-only permissions
            let error_msg = err.to_string();
            if error_msg.contains("can only be granted to admin users") {
                return (
                    StatusCode::BAD_REQUEST,
                    Json("Sensitive permissions (approve_withdrawal, manage_address, add_address, archive_address) can only be granted to admin users"),
                )
                    .into_response();
            }

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to grant wallet permission"),
            )
                .into_response()
        }
    }
}

/// List permissions for a wallet address
#[utoipa::path(
    get,
    path = "/api/v1/companies/{company_id}/wallets/{wallet_id}/permissions",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("wallet_id" = Uuid, Path, description = "Wallet address ID")
    ),
    responses(
        (status = 200, description = "Permissions retrieved successfully", body = Vec<WalletPermissionResponse>),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Wallet address not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn list_wallet_permissions(
    Path((company_id, wallet_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    // Verify wallet belongs to company
    match wallet::get_wallet_address_by_id(&state.pool, wallet_id).await {
        Ok(w) if w.company_id != company_id => {
            return (
                StatusCode::NOT_FOUND,
                Json("Wallet address not found in this company"),
            )
                .into_response()
        }
        Err(_) => return (StatusCode::NOT_FOUND, Json("Wallet address not found")).into_response(),
        _ => {}
    }

    match wallet::list_wallet_permissions(&state.pool, wallet_id).await {
        Ok(permissions) => {
            let response: Vec<WalletPermissionResponse> =
                permissions.into_iter().map(|p| p.into()).collect();
            Json(response).into_response()
        }
        Err(err) => {
            error!(error = %err, wallet_id = %wallet_id, "Failed to list wallet permissions");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to list wallet permissions"),
            )
                .into_response()
        }
    }
}

/// Revoke a wallet permission
#[utoipa::path(
    delete,
    path = "/api/v1/companies/{company_id}/wallets/permissions/{permission_id}",
    params(
        ("company_id" = Uuid, Path, description = "Company ID"),
        ("permission_id" = Uuid, Path, description = "Permission ID")
    ),
    responses(
        (status = 204, description = "Permission revoked successfully"),
        (status = 403, description = "Access denied"),
        (status = 404, description = "Permission not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Company Wallets"
)]
pub async fn revoke_wallet_permission(
    Path((_company_id, permission_id)): Path<(Uuid, Uuid)>,
    State(state): State<ApiState>,
) -> impl IntoResponse {
    // TODO: Add validation that permission belongs to this company

    match wallet::revoke_wallet_permission(&state.pool, permission_id).await {
        Ok(_) => {
            info!(permission_id = %permission_id, "Wallet permission revoked successfully");
            StatusCode::NO_CONTENT.into_response()
        }
        Err(err) => {
            error!(error = %err, permission_id = %permission_id, "Failed to revoke wallet permission");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json("Failed to revoke wallet permission"),
            )
                .into_response()
        }
    }
}
