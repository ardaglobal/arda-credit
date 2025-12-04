//! Detection result handlers for collateral intelligence integration
//!
//! Receives detection results from arda-collateral-intel service

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::config::types::{ApiState, ErrorResponse};

// ============================================================================
// REQUEST/RESPONSE TYPES
// ============================================================================

/// Detection result from arda-collateral-intel service
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DetectionResultRequest {
    pub collateral_id: Uuid,
    pub detection_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highest_match_score: Option<f32>,
    pub match_count: usize,
    pub matches: Vec<DetectionMatch>,
    pub analyzed_at: DateTime<Utc>,
}

/// Individual match from detection
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DetectionMatch {
    pub matched_collateral_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_deal_id: Option<Uuid>,
    pub overall_score: f32,
    pub confidence: String,
    pub is_same_deal: bool,
    pub both_deals_active: bool,
    pub summary: String,
}

/// Success response for detection result
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DetectionResultResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// HANDLERS
// ============================================================================

/// Receive detection result from arda-collateral-intel service
///
/// This is an internal endpoint called by the intelligence service to report
/// detection results back to arda-credit.
#[utoipa::path(
    post,
    path = "/api/v1/internal/collateral/{id}/detection",
    params(
        ("id" = String, Path, description = "Collateral UUID")
    ),
    request_body = DetectionResultRequest,
    responses(
        (status = 200, description = "Detection result received successfully", body = DetectionResultResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Collateral not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "Internal",
    security(("bearer" = []))
)]
pub async fn receive_detection_result(
    State(_state): State<ApiState>,
    Path(id): Path<String>,
    Json(result): Json<DetectionResultRequest>,
) -> Result<Json<DetectionResultResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Parse collateral ID
    let collateral_id = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_id".to_string(),
                message: "Invalid collateral ID format".to_string(),
                details: None,
            }),
        )
    })?;

    // Validate that the collateral_id in path matches the one in request body
    if collateral_id != result.collateral_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "id_mismatch".to_string(),
                message: "Collateral ID in path does not match ID in request body".to_string(),
                details: None,
            }),
        ));
    }

    info!(
        "📥 Received detection result for collateral {}: status={}, matches={}",
        collateral_id, result.detection_status, result.match_count
    );

    // Log concerning matches
    let concerning_matches: Vec<_> = result
        .matches
        .iter()
        .filter(|m| m.both_deals_active && !m.is_same_deal)
        .collect();

    if !concerning_matches.is_empty() {
        warn!(
            "⚠️  Collateral {} has {} concerning matches (both deals active)",
            collateral_id,
            concerning_matches.len()
        );
        for m in concerning_matches {
            warn!(
                "   - Match: {} (score: {:.2}, confidence: {})",
                m.matched_collateral_id, m.overall_score, m.confidence
            );
        }
    }

    // TODO: Store detection results in database
    // Options:
    // 1. Update collateral table with detection_status, last_detection_at, highest_match_score
    // 2. Create separate collateral_detections table for full history
    // For now, we just log the results

    Ok(Json(DetectionResultResponse {
        success: true,
        message: format!("Detection result for collateral {} received", collateral_id),
    }))
}
