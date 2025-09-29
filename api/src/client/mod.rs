//! HTTP API Client for deal processing operations
//!
//! This client provides a typed interface for interacting with the deal processing API server.
//! The CLI uses this instead of direct database access.
//!
//! ## Recent Changes
//! Converted from loan-based terminology to deal-based terminology:
//! - Loan → Deal
//! - Borrower → Originator
//! - Lender → Investor
//! - All API endpoints and types updated accordingly

use chrono::{DateTime, Utc};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

use crate::paths;

// ============================================================================
// DEAL API TYPES (converted from loan types)
// ============================================================================

/// Import or define the request/response types
pub use crate::handlers::deals::deals::{
    CreateDealRequest,    // was CreateLoanRequest
    DealDetailsResponse,  // was LoanDetailsResponse
    DealListingResponse,  // was LoanListingResponse
    DealMarketplaceStats, // was MarketplaceStats

    DealPaymentHistoryResponse, // was LoanPaymentHistoryResponse
    DealStatsResponse,          // was LoanStatsResponse

    DealTransferResponse,           // was LoanTransferResponse
    DealTransferValidationResponse, // was TransferValidationResponse

    // Consolidated marketplace types
    ListDealForSaleRequest,  // was ListLoanForSaleRequest
    ListDealsQuery,          // was ListLoansQuery
    MarketplaceDealResponse, // was MarketplaceLoanResponse
    // Payment types
    RecordDealPaymentRequest, // was RecordPaymentRequest
    // Consolidated transfer types
    TransferDealOwnershipRequest, // was TransferLoanOwnershipRequest
    UpdateDealRequest,            // was UpdateLoanRequest
    ValidateDealTransferRequest,  // was ValidateTransferRequest
};

/// API client for the Arda Credit deal processing system
///
/// This client provides methods for interacting with all deal-related
/// endpoints including CRUD operations, payments, marketplace listings,
/// and ownership transfers.
///
/// ## Recent Changes
/// Converted from loan-based client to deal-based client:
/// - All loan methods → deal methods
/// - Marketplace functionality consolidated into deal client
/// - Transfer functionality consolidated into deal client
/// - Updated all endpoint URLs and field names
///
/// ## Usage Examples
///
/// ```rust
/// # use api::client::BatchApiClient;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let client = BatchApiClient::new("http://localhost:8080");
///
/// // Create a deal
/// let deal = client.create_deal(
///     "originator-id",
///     "investor-id",
///     "100000",
///     800,
///     Some(365),
///     None,
///     None
/// ).await?;
///
/// // List deals for sale
/// let deals = client.browse_deals_for_sale(Some(10), None).await?;
///
/// // Transfer deal ownership
/// let transfer = client.transfer_deal_ownership(
///     &deal.deal_id,
///     "current-investor-id",
///     "new-investor-id",
///     "95000"
/// ).await?;
/// # Ok(())
/// # }
/// ```
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

    #[error("Invalid input: {0}")]
    InvalidInput(String),
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

/// Generate invite code response
#[derive(Debug, Deserialize)]
pub struct GenerateInviteCodeResponse {
    pub code: String,
    pub max_uses: i32,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

/// Check invite code request
#[derive(Debug, Serialize)]
pub struct UseInviteCodeRequest {
    pub user_id: Option<String>,
}

/// Use invite code response
#[derive(Debug, Deserialize)]
pub struct UseInviteCodeResponse {
    pub code: String,
    pub is_valid: bool,
    pub was_used: bool,
    pub message: String,
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

/// User balance response
#[derive(Debug, Deserialize)]
pub struct UserBalanceResponse {
    pub user_id: Option<String>,
    pub wallet_address: String,
    pub available_balance: String,
    pub currency: String,
}

/// User transactions response
#[derive(Debug, Deserialize)]
pub struct UserTransactionsResponse {
    pub transactions: Vec<DepositTransactionInfo>,
    pub total_count: usize,
}

/// Deposit transaction info
#[derive(Debug, Deserialize)]
pub struct DepositTransactionInfo {
    pub id: i32,
    pub user_address: String,
    pub deposit_amount_usdc: Option<String>,
    pub status: String,
    pub transaction_hash: String,
    pub block_number: i64,
    pub confirmation_count: i32,
    pub required_confirmations: i32,
    pub created_at: Option<String>,
    pub processed_at: Option<String>,
}

/// Pool creation request
#[derive(Debug, Serialize)]
pub struct CreatePoolRequest {
    pub name: String,
    pub max_ltv_ratio: f64,
    pub max_interest_rate: f64,
    pub repayment_period_days: i32,
}

/// Pool response
#[derive(Debug, Deserialize)]
pub struct PoolResponse {
    pub pool_id: String,
    pub name: String,
    pub investor_id: String,
    pub max_ltv_ratio: f64,
    pub max_interest_rate: f64,
    pub repayment_period_days: i32,
    pub total_deposits: f64,
    pub available_liquidity: f64,
    pub utilized_liquidity: f64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Pools list response
#[derive(Debug, Deserialize)]
pub struct PoolsListResponse {
    pub pools: Vec<PoolResponse>,
    pub total_count: usize,
}

/// Pool status update request
#[derive(Debug, Serialize)]
pub struct UpdatePoolStatusRequest {
    pub status: String,
}

/// Pool matching request
#[derive(Debug, Serialize)]
pub struct FindMatchingPoolsRequest {
    pub required_liquidity: f64,
    pub max_ltv: f64,
    pub max_rate: f64,
    pub min_repayment_days: i32,
}

/// Transaction status response
#[derive(Debug, Deserialize)]
pub struct TransactionStatusResponse {
    pub found: bool,
    pub transaction: Option<DepositTransactionInfo>,
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

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Get the HTTP client
    pub fn http_client(&self) -> &Client {
        &self.client
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
        password: &str,
        jurisdiction: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::USER);
        let request = serde_json::json!({
            "user_type": user_type,
            "email": email,
            "password": password,
            "jurisdiction": jurisdiction.unwrap_or("US")
        });

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Register a new user with invite code (new method for simplified user types)
    pub async fn register_user(
        &self,
        invite_code: &str,
        email: &str,
        jurisdiction: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::USER);
        let request = serde_json::json!({
            "invite_code": invite_code,
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
        kyc_investor_type: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_kyc_path(user_id));
        let request = serde_json::json!({
            "kyc_data": {
                "document_type": document_type,
                "document_hash": document_hash,
                "jurisdiction": jurisdiction,
                "kyc_investor_type": kyc_investor_type
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

    /// Generate a new invite code
    pub async fn generate_invite_code(
        &self,
        user_type: &str,
        max_uses: Option<i32>,
        expires_in_hours: Option<i32>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::INVITE_CODE_GENERATE);
        let request = serde_json::json!({
            "user_type": user_type,
            "max_uses": max_uses,
            "expires_in_hours": expires_in_hours
        });
        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Validate if an invite code is valid (doesn't mark as used)
    /// Use this before showing the registration form to the user
    pub async fn validate_invite_code(
        &self,
        code: &str,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!(
            "{}{}",
            self.base_url,
            paths::invite_code_validate_path(code)
        );
        let response = self.client.post(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get user balance by wallet address
    pub async fn get_user_balance_by_address(
        &self,
        address: &str,
    ) -> Result<UserBalanceResponse, ApiClientError> {
        let url = format!(
            "{}{}",
            self.base_url,
            paths::user_balance_by_address_path(address)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get user balance by user ID
    pub async fn get_user_balance_by_id(
        &self,
        user_id: &str,
    ) -> Result<UserBalanceResponse, ApiClientError> {
        let url = format!(
            "{}{}",
            self.base_url,
            paths::user_balance_by_id_path(user_id)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get user transactions by user ID
    pub async fn get_user_transactions(
        &self,
        user_id: &str,
    ) -> Result<UserTransactionsResponse, ApiClientError> {
        let url = format!(
            "{}{}",
            self.base_url,
            paths::user_transactions_by_id_path(user_id)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get transaction status by hash
    pub async fn get_transaction_status(
        &self,
        tx_hash: &str,
    ) -> Result<TransactionStatusResponse, ApiClientError> {
        let url = format!(
            "{}{}",
            self.base_url,
            paths::transaction_by_hash_path(tx_hash)
        );
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update user wallet address
    pub async fn update_user_wallet(
        &self,
        user_id: &str,
        wallet_address: &str,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}{}", self.base_url, paths::user_path(user_id));
        let request = serde_json::json!({
            "wallet_address": wallet_address
        });

        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Register a new investor
    pub async fn register_investor(
        &self,
        email: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        jurisdiction: Option<&str>,
        accredited_investor: Option<bool>,
        investment_capacity: Option<f64>,
        regulatory_jurisdiction: Option<&str>,
        invite_code: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}/api/v1/auth/register/investor", self.base_url);
        let mut request = serde_json::json!({
            "email": email
        });

        if let Some(fn_val) = first_name {
            request["first_name"] = serde_json::Value::String(fn_val.to_string());
        }
        if let Some(ln_val) = last_name {
            request["last_name"] = serde_json::Value::String(ln_val.to_string());
        }
        if let Some(j_val) = jurisdiction {
            request["jurisdiction"] = serde_json::Value::String(j_val.to_string());
        }
        if let Some(ai_val) = accredited_investor {
            request["accredited_investor"] = serde_json::Value::Bool(ai_val);
        }
        if let Some(ic_val) = investment_capacity {
            request["investment_capacity"] = serde_json::json!(ic_val);
        }
        if let Some(rj_val) = regulatory_jurisdiction {
            request["regulatory_jurisdiction"] = serde_json::Value::String(rj_val.to_string());
        }
        if let Some(ic_val) = invite_code {
            request["invite_code"] = serde_json::Value::String(ic_val.to_string());
        }

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get investor profile
    pub async fn get_investor_profile(
        &self,
        user_id: &str,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}/api/v1/investor/profile", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("X-User-ID", user_id)
            .send()
            .await?;
        self.handle_response(response).await
    }

    /// Update investor profile
    pub async fn update_investor_profile(
        &self,
        user_id: &str,
        accredited_investor: Option<bool>,
        investment_capacity: Option<f64>,
        regulatory_jurisdiction: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}/api/v1/investor/profile", self.base_url);
        let mut request = serde_json::json!({});

        if let Some(ai_val) = accredited_investor {
            request["accredited_investor"] = serde_json::Value::Bool(ai_val);
        }
        if let Some(ic_val) = investment_capacity {
            request["investment_capacity"] = serde_json::json!(ic_val);
        }
        if let Some(rj_val) = regulatory_jurisdiction {
            request["regulatory_jurisdiction"] = serde_json::Value::String(rj_val.to_string());
        }

        let response = self
            .client
            .put(&url)
            .header("X-User-ID", user_id)
            .json(&request)
            .send()
            .await?;
        self.handle_response(response).await
    }

    // ============================================================================
    // LIQUIDITY POOL METHODS
    // ============================================================================

    /// Create a new liquidity pool
    pub async fn create_pool(
        &self,
        name: &str,
        max_ltv_ratio: f64,
        max_interest_rate: f64,
        repayment_period_days: i32,
    ) -> Result<PoolResponse, ApiClientError> {
        let url = format!("{}/api/v1/pools", self.base_url);
        let request = CreatePoolRequest {
            name: name.to_string(),
            max_ltv_ratio,
            max_interest_rate,
            repayment_period_days,
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// List liquidity pools with optional status filter
    pub async fn list_pools(
        &self,
        status_filter: Option<&str>,
    ) -> Result<PoolsListResponse, ApiClientError> {
        let mut url = format!("{}/api/pools", self.base_url);
        if let Some(status) = status_filter {
            url.push_str(&format!("?status={}", status));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get specific pool details
    pub async fn get_pool(&self, pool_id: &str) -> Result<Option<PoolResponse>, ApiClientError> {
        let url = format!("{}/api/pools/{}", self.base_url, pool_id);
        let response = self.client.get(&url).send().await?;

        match response.status().as_u16() {
            200 => Ok(Some(self.handle_response(response).await?)),
            404 => Ok(None),
            _ => Err(self.handle_error_response(response).await),
        }
    }

    /// Update pool status
    pub async fn update_pool_status(
        &self,
        pool_id: &str,
        status: &str,
    ) -> Result<PoolResponse, ApiClientError> {
        let url = format!("{}/api/pools/{}/status", self.base_url, pool_id);
        let request = UpdatePoolStatusRequest {
            status: status.to_string(),
        };

        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Find matching pools for auto-deployment
    pub async fn find_matching_pools(
        &self,
        required_liquidity: f64,
        max_ltv: f64,
        max_rate: f64,
        min_repayment_days: i32,
    ) -> Result<PoolsListResponse, ApiClientError> {
        let url = format!("{}/api/pools/match", self.base_url);
        let request = FindMatchingPoolsRequest {
            required_liquidity,
            max_ltv,
            max_rate,
            min_repayment_days,
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ============================================================================
    // DEAL MANAGEMENT METHODS (converted from loan methods)
    // ============================================================================

    /// Create a new deal (was create_loan)
    pub async fn create_deal(
        &self,
        originator_id: &str, // was borrower_id
        investor_id: &str,   // was lender_id
        principal_amount: &str,
        interest_rate_bps: u16,
        term_days: Option<u32>,
        metadata: Option<serde_json::Value>,
        custom_deal_id: Option<&str>,
    ) -> Result<DealDetailsResponse, ApiClientError> {
        // was LoanDetailsResponse
        let url = format!("{}/api/v1/deal", self.base_url); // was /loan
        let request = CreateDealRequest {
            // was CreateLoanRequest
            originator_id: originator_id.to_string(), // was borrower_id
            investor_id: investor_id.to_string(),     // was lender_id
            principal_amount: principal_amount.to_string(),
            interest_rate_bps,
            term_days,
            loan_status: None, // Optional status, defaults to "originated" if not provided
            metadata,
            custom_deal_id: custom_deal_id.map(|s| s.to_string()),
            status: None,
            industry: None,
            document_ids: None,
            title: None,
            description: None,
            risk_level: None,
            asset_class: None,
            region: None,
            minimum_investment: None,
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get deal details by ID (was get_loan_by_id)
    pub async fn get_deal_by_id(
        &self,
        deal_id: &str, // was loan_id
    ) -> Result<Option<DealDetailsResponse>, ApiClientError> {
        // was LoanDetailsResponse
        let url = format!("{}/api/v1/deal/{}", self.base_url, deal_id); // was /loan/{}
        let response = self.client.get(&url).send().await?;

        match response.status().as_u16() {
            200 => Ok(Some(self.handle_response(response).await?)),
            404 => Ok(None),
            _ => Err(self.handle_error_response(response).await),
        }
    }

    /// List deals with optional filtering (was list_loans)
    pub async fn list_deals(
        &self,
        status: Option<&str>,
        originator_id: Option<&str>, // was borrower_id
        investor_id: Option<&str>,   // was lender_id
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<DealDetailsResponse>, ApiClientError> {
        // was LoanDetailsResponse
        let mut url = format!("{}/api/v1/deals", self.base_url); // was /loans
        let mut params = Vec::new();

        if let Some(status) = status {
            params.push(format!("status={}", status));
        }
        if let Some(originator_id) = originator_id {
            // was borrower_id
            params.push(format!("originator_id={}", originator_id));
        }
        if let Some(investor_id) = investor_id {
            // was lender_id
            params.push(format!("investor_id={}", investor_id));
        }
        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }
        if let Some(offset) = offset {
            params.push(format!("offset={}", offset));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Update deal details (was update_loan)
    pub async fn update_deal(
        &self,
        deal_id: &str, // was loan_id
        principal_amount: Option<&str>,
        outstanding_balance: Option<&str>,
        interest_rate_bps: Option<u16>,
        loan_status: Option<&str>,            // keeping same field name
        maturity_date: Option<Option<&str>>, // Optional maturity date (ISO 8601), Option<Option<T>> to handle NULL
        custom_deal_id: Option<Option<&str>>, // Optional custom deal ID, Option<Option<T>> to handle NULL
    ) -> Result<DealDetailsResponse, ApiClientError> {
        // was LoanDetailsResponse
        let url = format!("{}/api/v1/deal/{}", self.base_url, deal_id); // was /loan/{}
        let request = UpdateDealRequest {
            // was UpdateLoanRequest
            principal_amount: principal_amount.map(|s| s.to_string()),
            outstanding_balance: outstanding_balance.map(|s| s.to_string()),
            interest_rate_bps,
            loan_status: loan_status.map(|s| s.to_string()), // keeping same field name
            maturity_date: maturity_date.map(|md| md.map(|s| s.to_string())),
            custom_deal_id: custom_deal_id.map(|opt| opt.map(|s| s.to_string())),
            title: None,
            description: None,
            risk_level: None,
            asset_class: None,
            region: None,
            minimum_investment: None,
        };

        let response = self.client.put(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Delete deal (was delete_loan)
    pub async fn delete_deal(
        &self,
        deal_id: &str, // was loan_id
    ) -> Result<(), ApiClientError> {
        let url = format!("{}/api/v1/deal/{}", self.base_url, deal_id); // was /loan/{}
        let response = self.client.delete(&url).send().await?;

        match response.status().as_u16() {
            200 | 204 => Ok(()),
            _ => Err(self.handle_error_response(response).await),
        }
    }

    /// Get deal statistics (was get_loan_stats)
    pub async fn get_deal_stats(&self) -> Result<DealStatsResponse, ApiClientError> {
        // was LoanStatsResponse
        let url = format!("{}/api/v1/deals/stats", self.base_url); // was /loans/stats
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ============================================================================
    // CONSOLIDATED MARKETPLACE METHODS (moved from marketplace client)
    // ============================================================================

    /// Browse deals available for sale (consolidated from marketplace client)
    pub async fn browse_deals_for_sale(
        &self,
        limit: Option<i32>,
        status: Option<&str>,
    ) -> Result<Vec<MarketplaceDealResponse>, ApiClientError> {
        // was MarketplaceLoanResponse
        let mut url = format!("{}/api/v1/deals/for-sale", self.base_url); // was /marketplace/loans
        let mut params = Vec::new();

        if let Some(limit) = limit {
            params.push(format!("limit={}", limit));
        }
        if let Some(status) = status {
            params.push(format!("status={}", status));
        }

        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// List a deal for sale (consolidated from marketplace client)
    pub async fn list_deal_for_sale(
        &self,
        deal_id: &str, // was loan_id
        seller_id: &str,
        asking_price: &str,
        description: Option<&str>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<DealListingResponse, ApiClientError> {
        // was LoanListingResponse
        let url = format!("{}/api/v1/deal/{}/list-for-sale", self.base_url, deal_id); // was /marketplace/loan/{}
        let request = ListDealForSaleRequest {
            // was ListLoanForSaleRequest
            seller_id: Uuid::parse_str(seller_id)
                .map_err(|_| ApiClientError::InvalidInput("Invalid seller ID".to_string()))?,
            asking_price: asking_price.to_string(),
            description: description.map(|s| s.to_string()),
            expires_at,
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Remove deal from sale (consolidated from marketplace client)
    pub async fn remove_deal_from_sale(
        &self,
        deal_id: &str, // was loan_id
    ) -> Result<(), ApiClientError> {
        let url = format!("{}/api/v1/deal/{}/remove-from-sale", self.base_url, deal_id); // was /marketplace/loan/{}
        let response = self.client.delete(&url).send().await?;

        match response.status().as_u16() {
            200 | 204 => Ok(()),
            _ => Err(self.handle_error_response(response).await),
        }
    }

    /// Get deal marketplace statistics (consolidated from marketplace client)
    pub async fn get_deal_marketplace_stats(&self) -> Result<DealMarketplaceStats, ApiClientError> {
        // was MarketplaceStats
        let url = format!("{}/api/v1/deals/marketplace/stats", self.base_url); // was /marketplace/stats
        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    // ============================================================================
    // CONSOLIDATED TRANSFER METHODS (moved from transfer client)
    // ============================================================================

    /// Transfer deal ownership (consolidated from transfer client)
    pub async fn transfer_deal_ownership(
        &self,
        deal_id: &str,          // was loan_id
        from_investor_id: &str, // was from_lender_id
        to_investor_id: &str,   // was to_lender_id
        transfer_price: &str,
    ) -> Result<DealTransferResponse, ApiClientError> {
        // was LoanTransferResponse
        let url = format!("{}/api/v1/deal/{}/transfer", self.base_url, deal_id); // was /transfer/loan/{}
        let request = TransferDealOwnershipRequest {
            // was TransferLoanOwnershipRequest
            from_investor_id: Uuid::parse_str(from_investor_id) // was from_lender_id
                .map_err(|_| {
                    ApiClientError::InvalidInput("Invalid from investor ID".to_string())
                })?,
            to_investor_id:
                Uuid::parse_str(to_investor_id) // was to_lender_id
                    .map_err(|_| {
                        ApiClientError::InvalidInput("Invalid to investor ID".to_string())
                    })?,
            transfer_price: transfer_price.to_string(),
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get deal transfer history (consolidated from transfer client)
    pub async fn get_deal_transfer_history(
        &self,
        deal_id: &str, // was loan_id
        limit: Option<i32>,
    ) -> Result<Vec<DealTransferResponse>, ApiClientError> {
        // was LoanTransferResponse
        let mut url = format!("{}/api/v1/deal/{}/transfers", self.base_url, deal_id); // was /loan/{}/transfers

        if let Some(limit) = limit {
            url.push_str(&format!("?limit={}", limit));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Get user's deal transfer history (consolidated from transfer client)
    pub async fn get_user_deal_transfer_history(
        &self,
        user_id: &str,
        limit: Option<i32>,
    ) -> Result<Vec<DealTransferResponse>, ApiClientError> {
        // was LoanTransferResponse
        let mut url = format!("{}/api/v1/user/{}/deal-transfers", self.base_url, user_id); // was /user/{}/transfers

        if let Some(limit) = limit {
            url.push_str(&format!("?limit={}", limit));
        }

        let response = self.client.get(&url).send().await?;
        self.handle_response(response).await
    }

    /// Validate deal transfer (consolidated from transfer client)
    pub async fn validate_deal_transfer(
        &self,
        deal_id: &str,             // was loan_id
        current_investor_id: &str, // was current_lender_id
    ) -> Result<DealTransferValidationResponse, ApiClientError> {
        // was TransferValidationResponse
        let url = format!(
            "{}/api/v1/deal/{}/transfer/validate",
            self.base_url, deal_id
        ); // was /transfer/loan/{}/validate
        let request = ValidateDealTransferRequest {
            // was ValidateTransferRequest
            current_investor_id: Uuid::parse_str(current_investor_id) // was current_lender_id
                .map_err(|_| {
                    ApiClientError::InvalidInput("Invalid current investor ID".to_string())
                })?,
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    // ============================================================================
    // DEAL PAYMENT METHODS (updated from loan payment methods)
    // ============================================================================

    /// Record a deal payment (was record_loan_payment)
    pub async fn record_deal_payment(
        &self,
        deal_id: &str, // was loan_id
        payer_id: &str,
        amount: &str,
        payment_type: &str,
        payment_reference: Option<&str>,
        notes: Option<&str>,
    ) -> Result<serde_json::Value, ApiClientError> {
        let url = format!("{}/api/v1/payments", self.base_url); // URL might stay the same
        let request = RecordDealPaymentRequest {
            // was RecordPaymentRequest
            deal_id: deal_id.to_string(), // was loan_id
            amount: amount.to_string(),
            payment_type: payment_type.to_string(),
            payment_reference: payment_reference.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
        };

        let response = self.client.post(&url).json(&request).send().await?;
        self.handle_response(response).await
    }

    /// Get deal payment history (was get_loan_payment_history)
    pub async fn get_deal_payment_history(
        &self,
        deal_id: &str, // was loan_id
        limit: Option<i32>,
    ) -> Result<DealPaymentHistoryResponse, ApiClientError> {
        // was LoanPaymentHistoryResponse
        let mut url = format!("{}/api/v1/deal/{}/payments", self.base_url, deal_id); // was /loan/{}/payments

        if let Some(limit) = limit {
            url.push_str(&format!("?limit={}", limit));
        }

        let response = self.client.get(&url).send().await?;
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

        match status {
            400 => ApiClientError::ApiError {
                status,
                message: format!("Bad request for deal operation: {}", message), // was loan operation
            },
            404 => ApiClientError::ApiError {
                status,
                message: format!("Deal not found: {}", message), // was Loan not found
            },
            500 => ApiClientError::ApiError {
                status,
                message: format!("Server error in deal processing: {}", message), // was loan processing
            },
            _ => ApiClientError::ApiError { status, message },
        }
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
