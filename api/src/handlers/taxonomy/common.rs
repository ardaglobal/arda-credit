//! Common types and utilities for taxonomy API handlers
//!
//! This module provides shared request/response types, validation helpers,
//! and error handling utilities used across all taxonomy endpoints.

use axum::http::StatusCode;
use axum::response::Json;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::config::types::ErrorResponse;

/// Standard pagination and filtering parameters for list endpoints
#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct ListQueryParams {
    /// Maximum number of records to return (default: 100, max: 1000)
    #[param(example = 100)]
    pub limit: Option<i64>,

    /// Number of records to skip (default: 0)
    #[param(example = 0)]
    pub offset: Option<i64>,

    /// Sort field (e.g., "created_at", "document_id")
    #[param(example = "created_at")]
    pub sort_by: Option<String>,

    /// Sort direction: "asc" or "desc" (default: "desc")
    #[param(example = "desc")]
    pub sort_order: Option<String>,
}

impl ListQueryParams {
    /// Get limit with default and max values applied
    pub fn get_limit(&self) -> i64 {
        self.limit.unwrap_or(100).min(1000).max(1)
    }

    /// Get offset with default applied
    pub fn get_offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }

    /// Get sort order as SQL keyword
    pub fn get_sort_order(&self) -> &str {
        match self.sort_order.as_deref() {
            Some("asc") | Some("ASC") => "ASC",
            _ => "DESC",
        }
    }
}

/// Standard paginated response wrapper
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PaginatedResponse<T> {
    /// The data items for this page
    pub data: Vec<T>,

    /// Total count of items (across all pages)
    #[schema(example = 150)]
    pub total: i64,

    /// Number of items returned in this response
    #[schema(example = 20)]
    pub count: i64,

    /// The limit used for this request
    #[schema(example = 20)]
    pub limit: i64,

    /// The offset used for this request
    #[schema(example = 0)]
    pub offset: i64,
}

/// Standard success response for delete operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DeleteResponse {
    /// Whether the deletion was successful
    #[schema(example = true)]
    pub success: bool,

    /// A message describing the result
    #[schema(example = "Record deleted successfully")]
    pub message: String,

    /// The ID of the deleted record
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: String,
}

/// Standard success response for batch operations
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchOperationResponse {
    /// Number of records successfully processed
    #[schema(example = 15)]
    pub success_count: usize,

    /// Number of records that failed processing
    #[schema(example = 2)]
    pub error_count: usize,

    /// Total number of records in the batch
    #[schema(example = 17)]
    pub total: usize,

    /// IDs of successfully created/updated records
    pub successful_ids: Vec<String>,

    /// Details of any errors that occurred
    pub errors: Vec<BatchError>,
}

/// Details of an error in a batch operation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BatchError {
    /// Index of the item in the batch that failed
    #[schema(example = 3)]
    pub index: usize,

    /// Error code
    #[schema(example = "invalid_uuid")]
    pub error_code: String,

    /// Human-readable error message
    #[schema(example = "Invalid UUID format for credit_agreement_id")]
    pub error_message: String,
}

/// Helper function to parse UUID from string with standardized error handling
pub fn parse_uuid(
    id_str: &str,
    field_name: &str,
) -> Result<Uuid, (StatusCode, Json<ErrorResponse>)> {
    Uuid::parse_str(id_str).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_uuid",
                &format!("Invalid UUID format for {}", field_name),
            )),
        )
    })
}

/// Helper function to parse optional UUID
pub fn parse_optional_uuid(
    id_str: &Option<String>,
    field_name: &str,
) -> Result<Option<Uuid>, (StatusCode, Json<ErrorResponse>)> {
    match id_str {
        Some(s) => parse_uuid(s, field_name).map(Some),
        None => Ok(None),
    }
}

/// Helper function to validate decimal value
pub fn validate_decimal(
    value: &Decimal,
    field_name: &str,
    min: Option<Decimal>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if let Some(min_val) = min {
        if *value < min_val {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse::new(
                    "invalid_value",
                    &format!("{} must be at least {}", field_name, min_val),
                )),
            ));
        }
    }
    Ok(())
}

/// Helper to create internal server error response
pub fn internal_error(
    operation: &str,
    error: impl std::fmt::Display,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse::new(
            "internal_error",
            &format!("{} failed: {}", operation, error),
        )),
    )
}

/// Helper to create not found error response
pub fn not_found_error(resource: &str, id: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse::new(
            "not_found",
            &format!("{} with ID {} not found", resource, id),
        )),
    )
}

/// Helper to create bad request error response
pub fn bad_request_error(field: &str, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse::new(
            "bad_request",
            &format!("{}: {}", field, message),
        )),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_query_params_defaults() {
        let params = ListQueryParams {
            limit: None,
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        assert_eq!(params.get_limit(), 100);
        assert_eq!(params.get_offset(), 0);
        assert_eq!(params.get_sort_order(), "DESC");
    }

    #[test]
    fn test_list_query_params_max_limit() {
        let params = ListQueryParams {
            limit: Some(5000),
            offset: None,
            sort_by: None,
            sort_order: None,
        };
        assert_eq!(params.get_limit(), 1000);
    }

    #[test]
    fn test_parse_uuid_valid() {
        let result = parse_uuid("550e8400-e29b-41d4-a716-446655440000", "test_field");
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_uuid_invalid() {
        let result = parse_uuid("invalid-uuid", "test_field");
        assert!(result.is_err());
    }
}
