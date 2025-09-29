//! Core API types and configurations
//!
//! This module contains the fundamental types and configurations used across
//! the API, extracted from the original rest.rs file.

use crate::{
    services::auth::AuthService, services::balance_service::BalanceVerificationService,
    services::batch_processor::BatchProcessorHandle,
    services::withdrawal_service::WithdrawalTransactionService,
};
use arda_credit_db::IndexedMerkleTreeADS;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::{IntoParams, ToSchema};

// Re-export the core types from router.rs
pub use crate::api::router::{ApiConfig, ApiState};

// ============================================================================
// CORE TYPES MOVED FROM REST.RS
// ============================================================================

/// User type enumeration for allowed user types
#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserType {
    Investor,
    Originator,
    Admin,
}

impl std::fmt::Display for UserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Investor => write!(f, "investor"),
            Self::Originator => write!(f, "originator"),
            Self::Admin => write!(f, "admin"),
        }
    }
}

impl std::str::FromStr for UserType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "investor" => Ok(Self::Investor),
            "originator" => Ok(Self::Originator),
            "admin" => Ok(Self::Admin),
            _ => Err(format!("Invalid user type: {}", s)),
        }
    }
}

/// Standard error response format
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl ErrorResponse {
    pub fn new(error: &str, message: &str) -> Self {
        Self {
            error: error.to_string(),
            message: message.to_string(),
            details: None,
        }
    }
}

// Note: All types are now imported from rest.rs to avoid duplication
// This file serves as a bridge during the transition period
