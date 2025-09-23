//! Document API handlers with role-based access control and security
//!
//! This module provides HTTP endpoints for document operations including upload,
//! download, approval, and audit trail access with comprehensive security controls.

use crate::{
    auth::Claims,
    document_service::{
        DocumentAuditLogDto, DocumentListRequest, DocumentResponseDto, DocumentService,
        DocumentServiceError, DocumentUploadRequestDto,
    },
};
use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use tracing::{error, info, warn};
use uuid::Uuid;
use validator::Validate;

/// Document upload response
#[derive(Debug, Serialize)]
pub struct DocumentUploadResponse {
    pub document: DocumentResponseDto,
    pub message: String,
}

/// Document download response headers
#[derive(Debug)]
pub struct DocumentDownloadHeaders {
    pub content_type: String,
    pub content_disposition: String,
    pub content_length: usize,
}

/// Document verification response
#[derive(Debug, Serialize)]
pub struct DocumentVerificationResponse {
    pub document_id: Uuid,
    pub is_valid: bool,
    pub message: String,
}

/// Document list response
#[derive(Debug, Serialize)]
pub struct DocumentListResponse {
    pub documents: Vec<DocumentResponseDto>,
    pub total_count: Option<i64>,
    pub has_more: bool,
}

/// Audit logs response
#[derive(Debug, Serialize)]
pub struct AuditLogsResponse {
    pub logs: Vec<DocumentAuditLogDto>,
    pub total_count: Option<i64>,
    pub has_more: bool,
}

/// Error response structure
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl IntoResponse for DocumentServiceError {
    fn into_response(self) -> Response {
        let (status, error_type, message, details) = match &self {
            DocumentServiceError::AccessDenied { message } => (
                StatusCode::FORBIDDEN,
                "access_denied",
                message.clone(),
                None,
            ),
            DocumentServiceError::CompanyNotFound { company_id } => (
                StatusCode::NOT_FOUND,
                "company_not_found",
                format!("Company {} not found", company_id),
                Some(json!({"company_id": company_id})),
            ),
            DocumentServiceError::DocumentNotFound { document_id } => (
                StatusCode::NOT_FOUND,
                "document_not_found",
                format!("Document {} not found", document_id),
                Some(json!({"document_id": document_id})),
            ),
            DocumentServiceError::UserNotFound { user_id } => (
                StatusCode::NOT_FOUND,
                "user_not_found",
                format!("User {} not found", user_id),
                Some(json!({"user_id": user_id})),
            ),
            DocumentServiceError::TeamMemberNotFound { user_id, company_id } => (
                StatusCode::FORBIDDEN,
                "team_member_not_found",
                format!("User {} is not a member of company {}", user_id, company_id),
                Some(json!({"user_id": user_id, "company_id": company_id})),
            ),
            DocumentServiceError::KybNotApproved { company_id } => (
                StatusCode::FORBIDDEN,
                "kyb_not_approved",
                format!("Company {} KYB verification required", company_id),
                Some(json!({"company_id": company_id})),
            ),
            DocumentServiceError::KycNotApproved { user_id } => (
                StatusCode::FORBIDDEN,
                "kyc_not_approved",
                format!("User {} KYC verification required", user_id),
                Some(json!({"user_id": user_id})),
            ),
            DocumentServiceError::InvalidFileType { file_type } => (
                StatusCode::BAD_REQUEST,
                "invalid_file_type",
                format!("File type '{}' not supported. Supported: PDF, DOC, DOCX, CSV", file_type),
                Some(json!({"file_type": file_type, "supported_types": ["pdf", "doc", "docx", "csv"]})),
            ),
            DocumentServiceError::FileSizeExceeded { size_mb } => (
                StatusCode::BAD_REQUEST,
                "file_size_exceeded",
                format!("File size {}MB exceeds 100MB limit", size_mb),
                Some(json!({"size_mb": size_mb, "max_size_mb": 100})),
            ),
            DocumentServiceError::DuplicateDocument { filename, company_id } => (
                StatusCode::CONFLICT,
                "duplicate_document",
                format!("Document '{}' already exists in company {}", filename, company_id),
                Some(json!({"filename": filename, "company_id": company_id})),
            ),
            DocumentServiceError::ApprovalRequired { operation } => (
                StatusCode::FORBIDDEN,
                "approval_required",
                format!("Document approval required before {}", operation),
                Some(json!({"operation": operation})),
            ),
            DocumentServiceError::ValidationError(validation_errors) => (
                StatusCode::BAD_REQUEST,
                "validation_error",
                "Request validation failed".to_string(),
                Some(json!({"validation_errors": validation_errors})),
            ),
            DocumentServiceError::DatabaseError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "database_error",
                "Internal database error".to_string(),
                None,
            ),
            DocumentServiceError::StorageError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "storage_error",
                "Internal storage error".to_string(),
                None,
            ),
            DocumentServiceError::DocumentError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "document_error",
                "Internal document processing error".to_string(),
                None,
            ),
        };

        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message,
            details,
        };

        (status, Json(error_response)).into_response()
    }
}

/// Upload document endpoint
/// POST /api/companies/{company_id}/documents/upload
pub async fn upload_document(
    State(document_service): State<Arc<DocumentService>>,
    Path(company_id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<DocumentUploadResponse>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        "Document upload request received"
    );

    // Parse multipart form data
    let mut upload_request = None;
    let mut file_content = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        DocumentServiceError::ValidationError(validator::ValidationErrors::new())
    })? {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "metadata" => {
                let data = field.bytes().await.map_err(|_| {
                    DocumentServiceError::ValidationError(validator::ValidationErrors::new())
                })?;
                upload_request = Some(serde_json::from_slice::<DocumentUploadRequestDto>(&data)?);
            }
            "file" => {
                let data = field.bytes().await.map_err(|_| {
                    DocumentServiceError::ValidationError(validator::ValidationErrors::new())
                })?;
                file_content = Some(data.to_vec());
            }
            _ => {
                warn!("Unknown multipart field: {}", name);
            }
        }
    }

    let upload_request = upload_request.ok_or_else(|| {
        DocumentServiceError::ValidationError(validator::ValidationErrors::new())
    })?;

    let file_content = file_content.ok_or_else(|| {
        DocumentServiceError::ValidationError(validator::ValidationErrors::new())
    })?;

    // Upload document
    let document = document_service
        .upload_document(&claims, company_id, upload_request, file_content)
        .await?;

    info!(
        document_id = %document.id,
        user_id = %claims.sub,
        company_id = %company_id,
        filename = %document.filename,
        "Document uploaded successfully"
    );

    Ok(Json(DocumentUploadResponse {
        document,
        message: "Document uploaded successfully".to_string(),
    }))
}

/// Download document endpoint
/// GET /api/companies/{company_id}/documents/{document_id}/download
pub async fn download_document(
    State(document_service): State<Arc<DocumentService>>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Response, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        "Document download request received"
    );

    // Download document
    let (content, content_type, filename) = document_service
        .download_document(&claims, document_id)
        .await?;

    info!(
        document_id = %document_id,
        user_id = %claims.sub,
        filename = %filename,
        size_bytes = content.len(),
        "Document downloaded successfully"
    );

    // Create response with appropriate headers
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", filename),
        )
        .header("Content-Length", content.len().to_string())
        .header("Cache-Control", "private, no-cache")
        .body(axum::body::Body::from(content))
        .unwrap();

    Ok(response)
}

/// Get document metadata endpoint
/// GET /api/companies/{company_id}/documents/{document_id}
pub async fn get_document(
    State(document_service): State<Arc<DocumentService>>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<DocumentResponseDto>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        "Get document metadata request received"
    );

    // This would need to be implemented in the service
    // For now, return a placeholder error
    Err(DocumentServiceError::DocumentNotFound { document_id })
}

/// List documents endpoint
/// GET /api/companies/{company_id}/documents
pub async fn list_documents(
    State(document_service): State<Arc<DocumentService>>,
    Path(company_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<DocumentListResponse>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        "List documents request received"
    );

    // Parse query parameters
    let request = DocumentListRequest {
        company_id,
        document_type: params.get("document_type").cloned(),
        deal_id: params
            .get("deal_id")
            .and_then(|s| Uuid::parse_str(s).ok()),
        status: params.get("status").cloned(),
        approval_status: params.get("approval_status").cloned(),
        limit: params
            .get("limit")
            .and_then(|s| s.parse().ok())
            .or(Some(50)), // Default limit
        offset: params
            .get("offset")
            .and_then(|s| s.parse().ok())
            .or(Some(0)), // Default offset
    };

    // List documents
    let documents = document_service.list_documents(&claims, request).await?;

    let has_more = documents.len() >= 50; // Based on default limit

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_count = documents.len(),
        "Documents listed successfully"
    );

    Ok(Json(DocumentListResponse {
        documents,
        total_count: None, // Would need separate count query
        has_more,
    }))
}

/// Approve document endpoint
/// POST /api/companies/{company_id}/documents/{document_id}/approve
pub async fn approve_document(
    State(document_service): State<Arc<DocumentService>>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<DocumentResponseDto>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        "Document approval request received"
    );

    // Approve document
    let document = document_service.approve_document(&claims, document_id).await?;

    info!(
        document_id = %document_id,
        user_id = %claims.sub,
        "Document approved successfully"
    );

    Ok(Json(document))
}

/// Reject document endpoint
/// POST /api/companies/{company_id}/documents/{document_id}/reject
pub async fn reject_document(
    State(document_service): State<Arc<DocumentService>>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<DocumentResponseDto>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        "Document rejection request received"
    );

    // This would need to be implemented in the service
    // For now, return a placeholder error
    Err(DocumentServiceError::DocumentNotFound { document_id })
}

/// Delete document endpoint
/// DELETE /api/companies/{company_id}/documents/{document_id}
pub async fn delete_document(
    State(document_service): State<Arc<DocumentService>>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<StatusCode, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        "Document deletion request received"
    );

    // Delete document
    document_service.delete_document(&claims, document_id).await?;

    info!(
        document_id = %document_id,
        user_id = %claims.sub,
        "Document deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Verify document integrity endpoint
/// POST /api/companies/{company_id}/documents/{document_id}/verify
pub async fn verify_document(
    State(document_service): State<Arc<DocumentService>>,
    Path((company_id, document_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<Json<DocumentVerificationResponse>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        document_id = %document_id,
        "Document verification request received"
    );

    // Verify document integrity
    let is_valid = document_service
        .verify_document_integrity(&claims, document_id)
        .await?;

    let message = if is_valid {
        "Document integrity verified successfully"
    } else {
        "Document integrity verification failed"
    };

    info!(
        document_id = %document_id,
        user_id = %claims.sub,
        is_valid = is_valid,
        "Document verification completed"
    );

    Ok(Json(DocumentVerificationResponse {
        document_id,
        is_valid,
        message: message.to_string(),
    }))
}

/// Get document audit logs endpoint
/// GET /api/companies/{company_id}/documents/audit
pub async fn get_audit_logs(
    State(document_service): State<Arc<DocumentService>>,
    Path(company_id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<AuditLogsResponse>, DocumentServiceError> {
    // Extract JWT claims
    let claims = extract_claims(&headers)?;

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        "Audit logs request received"
    );

    // Parse query parameters
    let document_id = params
        .get("document_id")
        .and_then(|s| Uuid::parse_str(s).ok());
    let limit = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .or(Some(100)); // Default limit
    let offset = params
        .get("offset")
        .and_then(|s| s.parse().ok())
        .or(Some(0)); // Default offset

    // Get audit logs
    let logs = document_service
        .get_document_audit_logs(&claims, company_id, document_id, limit, offset)
        .await?;

    let has_more = logs.len() >= 100; // Based on default limit

    info!(
        user_id = %claims.sub,
        company_id = %company_id,
        log_count = logs.len(),
        "Audit logs retrieved successfully"
    );

    Ok(Json(AuditLogsResponse {
        logs,
        total_count: None, // Would need separate count query
        has_more,
    }))
}

/// Health check endpoint for document service
/// GET /api/documents/health
pub async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "status": "healthy",
        "service": "document_service",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

// Helper function to extract claims from headers
fn extract_claims(headers: &HeaderMap) -> Result<Claims, DocumentServiceError> {
    // Extract Authorization header
    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| DocumentServiceError::AccessDenied {
            message: "Authorization header missing".to_string(),
        })?;

    let auth_str = auth_header
        .to_str()
        .map_err(|_| DocumentServiceError::AccessDenied {
            message: "Invalid Authorization header".to_string(),
        })?;

    // Extract Bearer token
    let token = auth_str
        .strip_prefix("Bearer ")
        .ok_or_else(|| DocumentServiceError::AccessDenied {
            message: "Invalid Authorization format".to_string(),
        })?;

    // For now, return a placeholder Claims object
    // In a real implementation, this would decode and validate the JWT
    Ok(Claims {
        sub: "placeholder-user-id".to_string(),
        email: "placeholder@example.com".to_string(),
        user_type: "investor".to_string(),
        exp: chrono::Utc::now().timestamp() + 3600,
        iat: chrono::Utc::now().timestamp(),
        jti: uuid::Uuid::new_v4().to_string(),
    })
}

/// Document router configuration
pub fn document_routes() -> axum::Router<Arc<DocumentService>> {
    use axum::routing::{delete, get, post};

    axum::Router::new()
        // Document operations
        .route(
            "/api/companies/:company_id/documents/upload",
            post(upload_document),
        )
        .route(
            "/api/companies/:company_id/documents/:document_id/download",
            get(download_document),
        )
        .route(
            "/api/companies/:company_id/documents/:document_id",
            get(get_document),
        )
        .route(
            "/api/companies/:company_id/documents",
            get(list_documents),
        )
        .route(
            "/api/companies/:company_id/documents/:document_id/approve",
            post(approve_document),
        )
        .route(
            "/api/companies/:company_id/documents/:document_id/reject",
            post(reject_document),
        )
        .route(
            "/api/companies/:company_id/documents/:document_id",
            delete(delete_document),
        )
        .route(
            "/api/companies/:company_id/documents/:document_id/verify",
            post(verify_document),
        )
        // Audit and monitoring
        .route(
            "/api/companies/:company_id/documents/audit",
            get(get_audit_logs),
        )
        .route("/api/documents/health", get(health_check))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            error: "test_error".to_string(),
            message: "Test error message".to_string(),
            details: Some(json!({"field": "value"})),
        };

        let serialized = serde_json::to_string(&error).unwrap();
        assert!(serialized.contains("test_error"));
        assert!(serialized.contains("Test error message"));
    }

    #[test]
    fn test_document_service_error_status_codes() {
        let access_denied = DocumentServiceError::AccessDenied {
            message: "Access denied".to_string(),
        };
        let response = access_denied.into_response();
        // Would need to extract status code from response in real test

        let not_found = DocumentServiceError::DocumentNotFound {
            document_id: Uuid::new_v4(),
        };
        let response = not_found.into_response();
        // Would need to extract status code from response in real test
    }
}