//! Core API types and configurations
//!
//! This module contains the fundamental types and configurations used across
//! the API, extracted from the original rest.rs file.

use crate::{
    auth::AuthService, batch_processor::BatchProcessorHandle, BalanceVerificationService,
    WithdrawalTransactionService,
};
use arda_credit_db::IndexedMerkleTreeADS;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::{IntoParams, ToSchema};

// Re-export the core types from rest.rs for now
pub use crate::rest::{ApiConfig, ApiState};

// Note: All types are now imported from rest.rs to avoid duplication
// This file serves as a bridge during the transition period
