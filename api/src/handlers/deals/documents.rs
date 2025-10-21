use crate::{
    api::router::ApiState,
    services::{auth::Claims, document_storage::UploadedFile},
};
use arda_credit_db::{
    create_document, create_ingestion_job, delete_document, get_deal_by_id,
    get_deal_document_stats, get_document_by_id, get_documents_by_deal_id, update_document,
    CreateDocumentRequest, DealDocument, StorageType, UpdateDocumentRequest,
};
use axum::{
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
use utoipa::ToSchema;
use uuid::Uuid;

/// Deal document response (API wrapper with ToSchema)
#[derive(Debug, Serialize, ToSchema)]
pub struct DealDocumentResponse {
    /// Unique document identifier - use this to reference the document
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub document_id: Uuid,

    /// Deal this document belongs to
    #[schema(example = "789e0123-e89b-12d3-a456-426614174000")]
    pub deal_id: Uuid,

    // File metadata
    pub file_name: String,
    pub original_file_name: String,
    pub file_type: String,
    pub file_extension: String,
    pub file_size_bytes: i64,

    // Storage location
    pub storage_path: String,
    pub storage_type: String,

    // Document categorization
    pub document_type: Option<String>,
    pub document_category: Option<String>,
    pub description: Option<String>,

    // Upload information
    pub uploaded_by: Uuid,
    pub uploaded_at: String,

    // Ingestion status
    pub ingestion_status: String,
    pub ingested_at: Option<String>,
    pub ingestion_error: Option<String>,

    // Extracted metadata
    pub page_count: Option<i32>,
    pub word_count: Option<i32>,
    pub sheet_count: Option<i32>,
    pub extracted_text_preview: Option<String>,

    // Security and validation
    pub checksum: String,
    pub is_deleted: bool,
    pub deleted_at: Option<String>,
    pub deleted_by: Option<Uuid>,

    // Version control (for document versioning)
    pub version: i32,
    pub parent_document_id: Option<Uuid>,

    // Audit timestamps
    pub created_at: String,
    pub updated_at: String,
}

impl From<DealDocument> for DealDocumentResponse {
    fn from(doc: DealDocument) -> Self {
        Self {
            document_id: doc.id, // Renamed from 'id' to 'document_id' for API clarity
            deal_id: doc.deal_id,
            file_name: doc.file_name,
            original_file_name: doc.original_file_name,
            file_type: doc.file_type,
            file_extension: doc.file_extension,
            file_size_bytes: doc.file_size_bytes,
            storage_path: doc.storage_path,
            storage_type: format!("{:?}", doc.storage_type).to_lowercase(),
            document_type: doc.document_type,
            document_category: doc.document_category,
            description: doc.description,
            uploaded_by: doc.uploaded_by,
            uploaded_at: doc.uploaded_at.to_rfc3339(),
            ingestion_status: format!("{:?}", doc.ingestion_status).to_lowercase(),
            ingested_at: doc.ingested_at.map(|t| t.to_rfc3339()),
            ingestion_error: doc.ingestion_error,
            page_count: doc.page_count,
            word_count: doc.word_count,
            sheet_count: doc.sheet_count,
            extracted_text_preview: doc.extracted_text_preview,
            checksum: doc.checksum,
            is_deleted: doc.is_deleted,
            deleted_at: doc.deleted_at.map(|t| t.to_rfc3339()),
            deleted_by: doc.deleted_by,
            version: doc.version,
            parent_document_id: doc.parent_document_id,
            created_at: doc.created_at.to_rfc3339(),
            updated_at: doc.updated_at.to_rfc3339(),
        }
    }
}

/// Document upload response
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentUploadResponse {
    pub document: DealDocumentResponse,
    pub message: String,
}

/// Document list response
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentListResponse {
    pub documents: Vec<DealDocumentResponse>,
    pub total_count: usize,
}

/// Document statistics response
#[derive(Debug, Serialize, ToSchema)]
pub struct DocumentStatsResponse {
    pub total_documents: i64,
    pub total_size_bytes: i64,
    pub pending_ingestion: i64,
    pub completed_ingestion: i64,
    pub failed_ingestion: i64,
}

/// Update document metadata request (API wrapper)
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdateDocumentMetadataRequest {
    pub document_type: Option<String>,
    pub document_category: Option<String>,
    pub description: Option<String>,
}

/// Upload document for a deal
/// POST /api/v1/deals/{deal_id}/documents
#[utoipa::path(
    post,
    path = "/api/v1/deals/{deal_id}/documents",
    params(
        ("deal_id" = Uuid, Path, description = "Deal ID")
    ),
    request_body(content = Vec<u8>, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Document uploaded successfully", body = DocumentUploadResponse),
        (status = 400, description = "Invalid request or file format"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Deal not found"),
        (status = 413, description = "File too large"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn upload_document(
    State(state): State<ApiState>,
    Path(deal_id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<DocumentUploadResponse>, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    info!(
        user_id = %user_id,
        deal_id = %deal_id,
        "Document upload request received"
    );

    // Verify deal exists and user has access
    let deal = get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal by checking company membership
    // originator_id and investor_id are company IDs, not user IDs
    let user_has_access = check_user_deal_access(&state.pool, user_id, &deal).await?;
    if !user_has_access {
        return Err(DocumentApiError::Forbidden(
            "You don't have access to this deal".to_string(),
        ));
    }

    // Parse multipart form data
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut document_type: Option<String> = None;
    let mut document_category: Option<String> = None;
    let mut description: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Failed to parse multipart field: {}", e);
        DocumentApiError::BadRequest("Invalid multipart data".to_string())
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                let data = field.bytes().await.map_err(|e| {
                    error!("Failed to read file data: {}", e);
                    DocumentApiError::BadRequest("Failed to read file data".to_string())
                })?;
                file_data = Some(data.to_vec());
            }
            "document_type" => {
                document_type = Some(field.text().await.map_err(|e| {
                    error!("Failed to read document_type: {}", e);
                    DocumentApiError::BadRequest("Invalid document_type".to_string())
                })?);
            }
            "document_category" => {
                document_category = Some(field.text().await.map_err(|e| {
                    error!("Failed to read document_category: {}", e);
                    DocumentApiError::BadRequest("Invalid document_category".to_string())
                })?);
            }
            "description" => {
                description = Some(field.text().await.map_err(|e| {
                    error!("Failed to read description: {}", e);
                    DocumentApiError::BadRequest("Invalid description".to_string())
                })?);
            }
            _ => {
                debug!("Unknown field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let file_data =
        file_data.ok_or_else(|| DocumentApiError::BadRequest("File is required".to_string()))?;
    let original_filename =
        filename.ok_or_else(|| DocumentApiError::BadRequest("Filename is required".to_string()))?;

    // Get MIME type from filename or content
    let mime_type = crate::services::document_storage::mime_from_extension(&original_filename)
        .unwrap_or_else(|| "application/octet-stream".to_string());

    // Get storage service
    let storage_service = &state.document_storage_service.as_ref().ok_or_else(|| {
        DocumentApiError::InternalError("Document storage service unavailable".into())
    })?;

    // Store the file
    let uploaded_file = UploadedFile {
        original_filename: original_filename.clone(),
        mime_type: mime_type.clone(),
        data: file_data,
    };

    let storage_metadata = storage_service
        .store_document(deal_id, uploaded_file)
        .await?;

    // Extract file extension from original filename
    let file_extension = std::path::Path::new(&original_filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| format!(".{}", s))
        .unwrap_or_else(|| ".bin".to_string());

    // Generate unique filename for storage
    let unique_filename = format!("{}_{}", Uuid::new_v4(), original_filename);

    // Clone document_type for SurrealDB before it gets moved
    let document_type_for_surreal = document_type
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    // Create document record in database
    let doc_request = CreateDocumentRequest {
        deal_id,
        file_name: unique_filename,
        original_file_name: original_filename.clone(),
        file_type: storage_metadata.mime_type.clone(),
        file_extension,
        file_size_bytes: storage_metadata.file_size_bytes as i64,
        storage_path: storage_metadata.storage_path.clone(),
        storage_type: match storage_metadata.storage_type {
            crate::services::document_storage::StorageBackend::Filesystem => {
                StorageType::Filesystem
            }
            crate::services::document_storage::StorageBackend::S3 => StorageType::S3,
        },
        document_type,
        document_category,
        description,
        uploaded_by: user_id,
        checksum: storage_metadata.checksum,
    };

    let document = create_document(&state.pool, doc_request).await?;

    // Store document metadata in SurrealDB for vector search and AI analysis
    if let Some(ref surrealdb_service) = state.surrealdb_service {
        info!(
            document_id = %document.id,
            deal_id = %deal_id,
            "Creating SurrealDB document record"
        );

        let surreal_doc = crate::services::document_ingestion::surrealdb_service::Document {
            document_id: document.id,
            company_id: deal.originator_id, // Use originator company as owner
            deal_id: Some(deal_id),
            filename: original_filename.clone(),
            document_type: document_type_for_surreal,
            file_size: storage_metadata.file_size_bytes as i64,
            user_id: Some(user_id),
            upload_timestamp: chrono::Utc::now(),
            status: "uploaded".to_string(),
            content: String::new(), // Will be populated during ingestion
            page_count: None,
            processing_progress: Some(0.0),
            error_message: None,
            retry_count: 0,
            processing_started_at: None,
            processing_completed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        if let Err(e) = surrealdb_service.create_document(surreal_doc).await {
            error!("Failed to create SurrealDB document record: {}", e);
            // Don't fail the upload if SurrealDB storage fails
            // The document is still in PostgreSQL and can be reprocessed
        } else {
            info!(
                document_id = %document.id,
                "SurrealDB document record created successfully"
            );
        }
    } else {
        debug!("SurrealDB service not available, skipping document metadata storage");
    }

    // Create ingestion job for async processing
    let _job = create_ingestion_job(&state.pool, document.id)
        .await
        .map_err(|e| {
            error!("Failed to create ingestion job: {}", e);
            // Don't fail the upload if ingestion job creation fails
            e
        })
        .ok();

    info!(
        document_id = %document.id,
        deal_id = %deal_id,
        user_id = %user_id,
        filename = %original_filename,
        size = storage_metadata.file_size_bytes,
        "Document uploaded successfully"
    );

    Ok(Json(DocumentUploadResponse {
        document: document.into(),
        message: "Document uploaded successfully".to_string(),
    }))
}

/// List documents for a deal
/// GET /api/v1/deals/{deal_id}/documents
#[utoipa::path(
    get,
    path = "/api/v1/deals/{deal_id}/documents",
    params(
        ("deal_id" = Uuid, Path, description = "Deal ID")
    ),
    responses(
        (status = 200, description = "Documents retrieved successfully", body = DocumentListResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_documents(
    State(state): State<ApiState>,
    Path(deal_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DocumentListResponse>, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    info!(
        user_id = %user_id,
        deal_id = %deal_id,
        "List documents request received"
    );

    // Verify deal exists and user has access
    let deal = get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal
    let user_has_access = check_user_deal_access(&state.pool, user_id, &deal).await?;
    if !user_has_access {
        return Err(DocumentApiError::Forbidden(
            "You don't have access to this deal".to_string(),
        ));
    }

    // Get documents
    let documents = get_documents_by_deal_id(&state.pool, deal_id).await?;

    info!(
        user_id = %user_id,
        deal_id = %deal_id,
        document_count = documents.len(),
        "Documents listed successfully"
    );

    Ok(Json(DocumentListResponse {
        total_count: documents.len(),
        documents: documents.into_iter().map(|d| d.into()).collect(),
    }))
}

/// Get document by ID
/// GET /api/v1/documents/{document_id}
#[utoipa::path(
    get,
    path = "/api/v1/documents/{document_id}",
    params(
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Document metadata retrieved successfully", body = DealDocumentResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Document not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Documents",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_document(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DealDocumentResponse>, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    // Get document
    let document = get_document_by_id(&state.pool, document_id).await?;

    // Verify deal exists and user has access
    let deal = get_deal_by_id(&state.pool, document.deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal
    let user_has_access = check_user_deal_access(&state.pool, user_id, &deal).await?;
    if !user_has_access {
        return Err(DocumentApiError::Forbidden(
            "You don't have access to this document".to_string(),
        ));
    }

    Ok(Json(document.into()))
}

/// Download document
/// GET /api/v1/documents/{document_id}/download
#[utoipa::path(
    get,
    path = "/api/v1/documents/{document_id}/download",
    params(
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    responses(
        (status = 200, description = "Document content", content_type = "application/octet-stream"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Document not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Documents",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn download_document(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    // Get document
    let document = get_document_by_id(&state.pool, document_id).await?;

    // Verify deal exists and user has access
    let deal = get_deal_by_id(&state.pool, document.deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal
    let user_has_access = check_user_deal_access(&state.pool, user_id, &deal).await?;
    if !user_has_access {
        return Err(DocumentApiError::Forbidden(
            "You don't have access to this document".to_string(),
        ));
    }

    // Get storage service
    let storage_service = &state.document_storage_service.as_ref().ok_or_else(|| {
        DocumentApiError::InternalError("Document storage service unavailable".into())
    })?;

    // Retrieve document from storage
    let content = storage_service
        .retrieve_document(
            &document.storage_path,
            storage_type_to_backend(document.storage_type),
        )
        .await?;

    info!(
        document_id = %document_id,
        user_id = %user_id,
        filename = %document.original_file_name,
        size = content.len(),
        "Document downloaded successfully"
    );

    // Create response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", &document.file_type)
        .header(
            "Content-Disposition",
            format!("attachment; filename=\"{}\"", document.original_file_name),
        )
        .header("Content-Length", content.len().to_string())
        .header("Cache-Control", "private, no-cache")
        .body(axum::body::Body::from(content))
        .unwrap();

    Ok(response)
}

/// Update document metadata
/// DEPRECATED: This endpoint is no longer exposed via the API router
/// Documents must be managed through deal context only
/// Use POST /api/v1/deals/{deal_id}/documents to upload new documents
///
/// PUT /api/v1/documents/{document_id}
#[deprecated(
    note = "Documents must be managed through deal context. Use POST /api/v1/deals/{deal_id}/documents"
)]
#[allow(dead_code)]
#[utoipa::path(
    put,
    path = "/api/v1/documents/{document_id}",
    params(
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    request_body = UpdateDocumentMetadataRequest,
    responses(
        (status = 200, description = "Document updated successfully", body = DealDocumentResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Document not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Documents",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_document_metadata(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<UpdateDocumentMetadataRequest>,
) -> Result<Json<DealDocumentResponse>, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    // Get document
    let document = get_document_by_id(&state.pool, document_id).await?;

    // Verify deal exists and user has access
    let _deal = get_deal_by_id(&state.pool, document.deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal (only uploader can update)
    if document.uploaded_by != user_id {
        return Err(DocumentApiError::Forbidden(
            "Only the uploader can update this document".to_string(),
        ));
    }

    // Convert API request to database request
    let db_req = UpdateDocumentRequest {
        document_type: req.document_type,
        document_category: req.document_category,
        description: req.description,
    };

    // Update document
    let updated_document = update_document(&state.pool, document_id, db_req).await?;

    info!(
        document_id = %document_id,
        user_id = %user_id,
        "Document metadata updated successfully"
    );

    Ok(Json(updated_document.into()))
}

/// Delete document
/// DELETE /api/v1/documents/{document_id}
#[utoipa::path(
    delete,
    path = "/api/v1/documents/{document_id}",
    params(
        ("document_id" = Uuid, Path, description = "Document ID")
    ),
    responses(
        (status = 204, description = "Document deleted successfully"),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Document not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Documents",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_document_handler(
    State(state): State<ApiState>,
    Path(document_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    // Get document
    let document = get_document_by_id(&state.pool, document_id).await?;

    // Verify deal exists and user has access
    let deal = get_deal_by_id(&state.pool, document.deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal (only uploader or deal parties can delete)
    let user_has_access = check_user_deal_access(&state.pool, user_id, &deal).await?;
    if document.uploaded_by != user_id && !user_has_access {
        return Err(DocumentApiError::Forbidden(
            "You don't have permission to delete this document".to_string(),
        ));
    }

    // Get storage service
    let storage_service = &state.document_storage_service.as_ref().ok_or_else(|| {
        DocumentApiError::InternalError("Document storage service unavailable".into())
    })?;

    // Delete from storage
    storage_service
        .delete_document(
            &document.storage_path,
            storage_type_to_backend(document.storage_type),
        )
        .await
        .map_err(|e| {
            error!("Failed to delete document from storage: {}", e);
            // Continue with database deletion even if storage deletion fails
            e
        })
        .ok();

    // Delete from database (soft delete)
    delete_document(&state.pool, document_id, user_id).await?;

    info!(
        document_id = %document_id,
        user_id = %user_id,
        "Document deleted successfully"
    );

    Ok(StatusCode::NO_CONTENT)
}

/// Get document statistics for a deal
/// GET /api/v1/deals/{deal_id}/documents/stats
#[utoipa::path(
    get,
    path = "/api/v1/deals/{deal_id}/documents/stats",
    params(
        ("deal_id" = Uuid, Path, description = "Deal ID")
    ),
    responses(
        (status = 200, description = "Document statistics retrieved successfully", body = DocumentStatsResponse),
        (status = 401, description = "Authentication required"),
        (status = 403, description = "Insufficient permissions"),
        (status = 404, description = "Deal not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Deals",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_document_stats(
    State(state): State<ApiState>,
    Path(deal_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DocumentStatsResponse>, DocumentApiError> {
    // Extract user from claims
    let claims = extract_claims(&state, &headers).await?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| DocumentApiError::Unauthorized)?;

    // Verify deal exists and user has access
    let deal = get_deal_by_id(&state.pool, deal_id)
        .await
        .map_err(|_| DocumentApiError::NotFound("Deal not found".to_string()))?
        .ok_or_else(|| DocumentApiError::NotFound("Deal not found".to_string()))?;

    // Check user has access to this deal
    let user_has_access = check_user_deal_access(&state.pool, user_id, &deal).await?;
    if !user_has_access {
        return Err(DocumentApiError::Forbidden(
            "You don't have access to this deal".to_string(),
        ));
    }

    // Get statistics
    let stats = get_deal_document_stats(&state.pool, deal_id).await?;

    Ok(Json(DocumentStatsResponse {
        total_documents: stats.total_documents,
        total_size_bytes: stats.total_size_bytes,
        pending_ingestion: stats.pending_ingestion,
        completed_ingestion: stats.completed_ingestion,
        failed_ingestion: stats.failed_ingestion,
    }))
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Check if user has access to a deal by verifying company membership
/// Returns true if user is a member of the originator or investor company
async fn check_user_deal_access(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    deal: &arda_credit_db::Deal,
) -> Result<bool, DocumentApiError> {
    // Get user's company IDs from active memberships
    let user_company_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT company_id
        FROM company_memberships
        WHERE user_id = $1 AND status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!("Failed to fetch user company memberships: {}", e);
        DocumentApiError::InternalError("Database error".to_string())
    })?;

    // Check if user is a member of the originator or investor company
    let has_access = user_company_ids.contains(&deal.originator_id)
        || deal
            .investor_id
            .map(|investor_id| user_company_ids.contains(&investor_id))
            .unwrap_or(false);

    Ok(has_access)
}

/// Convert database StorageType to service StorageBackend
fn storage_type_to_backend(
    storage_type: StorageType,
) -> crate::services::document_storage::StorageBackend {
    match storage_type {
        StorageType::Filesystem => crate::services::document_storage::StorageBackend::Filesystem,
        StorageType::S3 => crate::services::document_storage::StorageBackend::S3,
    }
}

/// Extract session token from cookie header
fn extract_session_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get("cookie")?.to_str().ok()?;

    // Parse cookies to find session_token
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(eq_pos) = cookie.find('=') {
            let (name, value) = cookie.split_at(eq_pos);
            if name.trim() == "session_token" {
                return Some(value[1..].to_string()); // Skip the '=' character
            }
        }
    }

    None
}

/// Extract and validate claims from session
async fn extract_claims(state: &ApiState, headers: &HeaderMap) -> Result<Claims, DocumentApiError> {
    // Extract session token from cookie header
    let session_token = extract_session_token_from_headers(headers)
        .ok_or_else(|| DocumentApiError::Unauthorized)?;

    // Validate session and get user identity
    let identity = state
        .session_service
        .validate_session(&session_token)
        .await
        .map_err(|e| {
            error!("Failed to validate session: {}", e);
            DocumentApiError::Unauthorized
        })?;

    // Convert identity to Claims
    Ok(Claims {
        sub: identity.id.to_string(),
        email: identity.email,
        user_type: identity.user_type,
        exp: 0,             // Not used for session-based auth
        iat: 0,             // Not used for session-based auth
        jti: String::new(), // Not used for session-based auth
    })
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug)]
pub enum DocumentApiError {
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    NotFound(String),
    InternalError(String),
    Database(arda_credit_db::DbError),
    Storage(crate::services::document_storage::ApiError),
}

impl From<arda_credit_db::DbError> for DocumentApiError {
    fn from(err: arda_credit_db::DbError) -> Self {
        DocumentApiError::Database(err)
    }
}

impl From<crate::services::document_storage::ApiError> for DocumentApiError {
    fn from(err: crate::services::document_storage::ApiError) -> Self {
        DocumentApiError::Storage(err)
    }
}

impl IntoResponse for DocumentApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            DocumentApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            DocumentApiError::Unauthorized => {
                (StatusCode::UNAUTHORIZED, "Unauthorized".to_string())
            }
            DocumentApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            DocumentApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            DocumentApiError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            DocumentApiError::Database(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Database error: {}", err),
            ),
            DocumentApiError::Storage(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Storage error: {}", err),
            ),
        };

        (
            status,
            Json(serde_json::json!({
                "error": message
            })),
        )
            .into_response()
    }
}
