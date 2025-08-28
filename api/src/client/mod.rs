//! HTTP API Client for batch processing operations
//!
//! This client provides a typed interface for interacting with the batch processing API server.
//! The CLI uses this instead of direct database access.

use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

use crate::paths;

/// Client for interacting with the batch processing API
#[derive(Debug, Clone)]
pub struct BatchApiClient {
    client: Client,
    base_url: String,
}

/// API client errors
#[derive(Error, Debug)]
pub enum ApiClientError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("API returned error: {status} - {message}")]
    ApiError { status: u16, message: String },

    #[error("Failed to serialize/deserialize: {0}")]
    SerializationError(#[from] serde_json::Error),
}

// ============================================================================
// REQUEST/RESPONSE TYPES (matching the API)
// ============================================================================

/// Request to submit a transaction
#[derive(Debug, Serialize)]
pub struct SubmitTransactionRequest {
    pub amount: i32,
}

/// Response from transaction submission
#[derive(Debug, Deserialize)]
pub struct SubmitTransactionResponse {
    pub transaction_id: i32,
    pub amount: i32,
    pub status: String,
    pub created_at: String,
}

/// Request to create a batch
#[derive(Debug, Serialize)]
pub struct CreateBatchRequest {
    pub batch_size: Option<i32>,
}

/// Response from batch creation
#[derive(Debug, Deserialize)]
pub struct CreateBatchResponse {
    pub batch_id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub created_at: String,
}

/// Response for pending transactions
#[derive(Debug, Deserialize)]
pub struct PendingTransactionsResponse {
    pub transactions: Vec<TransactionInfo>,
    pub total_count: usize,
    pub total_amount: i32,
}

/// Transaction information
#[derive(Debug, Deserialize)]
pub struct TransactionInfo {
    pub id: i32,
    pub amount: i32,
    pub created_at: String,
}

/// Response for batch listing
#[derive(Debug, Deserialize)]
pub struct BatchListResponse {
    pub batches: Vec<BatchInfo>,
    pub total_count: usize,
}

/// Batch information
#[derive(Debug, Deserialize)]
pub struct BatchInfo {
    pub id: i32,
    pub previous_counter_value: i64,
    pub final_counter_value: i64,
    pub transaction_count: usize,
    pub proof_status: String,
    pub sindri_proof_id: Option<String>,
    pub created_at: String,
    pub proven_at: Option<String>,
}

/// Response for current state
#[derive(Debug, Deserialize)]
pub struct CurrentStateResponse {
    pub counter_value: i64,
    pub has_merkle_root: bool,
    pub last_batch_id: Option<i32>,
    pub last_proven_batch_id: Option<i32>,
}

/// Request to update batch with proof
#[derive(Debug, Serialize)]
pub struct UpdateBatchProofRequest {
    pub sindri_proof_id: String,
    pub status: String,
    pub merkle_root: Option<String>,
}

/// Contract submission data (dry run)
#[derive(Debug, Deserialize)]
pub struct ContractSubmissionData {
    pub public: ContractPublicData,
    pub private: ContractPrivateData,
}

#[derive(Debug, Deserialize)]
pub struct ContractPublicData {
    pub prev_merkle_root: String,
    pub new_merkle_root: String,
    pub zk_proof: String,
}

#[derive(Debug, Deserialize)]
pub struct ContractPrivateData {
    pub prev_counter_value: i64,
    pub new_counter_value: i64,
    pub transactions: Vec<i32>,
}

/// Health check response
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
    pub database_connected: bool,
    pub api_version: String,
    pub git_tag: String,
    pub git_sha: String,
    pub git_sha_short: String,
    pub git_dirty: bool,
}

/// API info response
#[derive(Debug, Deserialize)]
pub struct ApiInfoResponse {
    pub server_name: String,
    pub version: String,
    pub timestamp: String,
    pub endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Deserialize)]
pub struct EndpointInfo {
    pub method: String,
    pub path: String,
    pub description: String,
}

// ============================================================================
// CLIENT IMPLEMENTATION
// ============================================================================

impl BatchApiClient {
    /// Create a new API client
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            base_url: base_url.into(),
        }
    }

    /// Submit a new transaction
    pub async fn submit_transaction(
        &self,
        amount: i32,
    ) -> Result<SubmitTransactionResponse, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::STATE_TRANSACTIONS);
        let request = SubmitTransactionRequest { amount };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get pending transactions
    pub async fn get_pending_transactions(
        &self,
    ) -> Result<PendingTransactionsResponse, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::STATE_TRANSACTIONS_PENDING);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Create a new batch
    pub async fn create_batch(
        &self,
        batch_size: Option<i32>,
    ) -> Result<CreateBatchResponse, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::STATE_BATCHES);
        let request = CreateBatchRequest { batch_size };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get all batches (paginated)
    pub async fn get_batches(
        &self,
        limit: Option<i32>,
    ) -> Result<BatchListResponse, ApiClientError> {
        let mut url = format!("{}{}", self.base_url, paths::STATE_BATCHES);
        if let Some(limit) = limit {
            url = format!("{}?limit={}", url, limit);
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get specific batch by ID
    pub async fn get_batch(&self, batch_id: i32) -> Result<Option<BatchInfo>, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::batch_path(batch_id));
        let response = self.client.get(&url).send().await?;

        match response.status().as_u16() {
            200 => Ok(Some(self.handle_response(response).await?)),
            404 => Ok(None),
            _ => Err(self.handle_error_response(response).await),
        }
    }

    /// Update batch with ZK proof
    pub async fn update_batch_proof(
        &self,
        batch_id: i32,
        sindri_proof_id: String,
        status: String,
        merkle_root: Option<String>,
    ) -> Result<BatchInfo, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::batch_proof_path(batch_id));
        let request = UpdateBatchProofRequest {
            sindri_proof_id,
            status,
            merkle_root,
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get current counter state
    pub async fn get_current_state(&self) -> Result<CurrentStateResponse, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::STATE_CURRENT);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get contract submission data (dry run)
    pub async fn get_contract_data(
        &self,
        batch_id: i32,
    ) -> Result<Option<ContractSubmissionData>, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::state_contract_path(batch_id));
        let response = self.client.get(&url).send().await?;

        match response.status().as_u16() {
            200 => Ok(Some(self.handle_response(response).await?)),
            404 => Ok(None),
            _ => Err(self.handle_error_response(response).await),
        }
    }

    /// Check API health
    pub async fn health_check(&self) -> Result<HealthResponse, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::API_HEALTH);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get API information
    pub async fn get_api_info(&self) -> Result<ApiInfoResponse, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::API_INFO);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Create a new user
    pub async fn create_user(
        &self,
        user_type: &str,
        email: &str,
        jurisdiction: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::USER);
        let request = serde_json::json!({
            "user_type": user_type,
            "email": email,
            "jurisdiction": jurisdiction.unwrap_or("US")
        });

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get user details by ID
    pub async fn get_user(&self, user_id: &str) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_path(user_id));
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// List users with optional filters
    pub async fn list_users(
        &self,
        user_type: Option<&str>,
        kyc_status: Option<&str>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let mut query_params = Vec::new();
        if let Some(ut) = user_type {
            query_params.push(format!("user_type={}", ut));
        }
        if let Some(ks) = kyc_status {
            query_params.push(format!("kyc_status={}", ks));
        }
        if let Some(l) = limit {
            query_params.push(format!("limit={}", l));
        }
        if let Some(o) = offset {
            query_params.push(format!("offset={}", o));
        }

        let query_string = if query_params.is_empty() {
            String::new()
        } else {
            format!("?{}", query_params.join("&"))
        };

        let url = format!("{}{}{}", self.base_url, paths::USERS, query_string);
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update user profile information
    pub async fn update_user(
        &self,
        user_id: &str,
        email: Option<&str>,
        user_type: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_path(user_id));
        let mut request = serde_json::Map::new();

        if let Some(e) = email {
            request.insert(
                "email".to_string(),
                serde_json::Value::String(e.to_string()),
            );
        }
        if let Some(ut) = user_type {
            request.insert(
                "user_type".to_string(),
                serde_json::Value::String(ut.to_string()),
            );
        }
        if let Some(j) = jurisdiction {
            request.insert(
                "jurisdiction".to_string(),
                serde_json::Value::String(j.to_string()),
            );
        }

        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete user by ID
    pub async fn delete_user(&self, user_id: &str) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_path(user_id));
        let response = self.client.delete(&url).send().await?;
        self.handle_response(response).await
    }

    /// Submit KYC information for a user
    pub async fn submit_kyc(
        &self,
        user_id: &str,
        document_type: &str,
        document_hash: &str,
        jurisdiction: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_kyc_path(user_id));
        let request = serde_json::json!({
            "kyc_data": {
                "document_type": document_type,
                "document_hash": document_hash,
                "jurisdiction": jurisdiction
            }
        });

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Update KYC status (admin only)
    pub async fn update_kyc_status(
        &self,
        user_id: &str,
        status: &str,
        notes: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_kyc_status_path(user_id));
        let request = serde_json::json!({
            "status": status,
            "notes": notes
        });

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Generic response handler
    async fn handle_response<T: for<'de> Deserialize<'de>>(
        &self,
        response: Response,
    ) -> Result<T, ApiClientError> {
        if response.status().is_success() {
            let json = response.json().await?;
            Ok(json)
        } else {
            Err(self.handle_error_response(response).await)
        }
    }

    /// Handle error responses
    async fn handle_error_response(&self, response: Response) -> ApiClientError {
        let status = response.status().as_u16();
        let message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        ApiClientError::ApiError { status, message }
    }
}

/// Default API client using localhost:8080
impl Default for BatchApiClient {
    fn default() -> Self {
        Self::new("http://localhost:8080")
    }
}

// Keep old client for backward compatibility (if needed during transition)
pub use BatchApiClient as ArithmeticApiClient;
