# BatchApiClient Method Reference

Quick reference for agents implementing API integration. All methods are available in `api/src/client/mod.rs`.

## 🔧 Available BatchApiClient Methods

### Core Batch Operations ✅ **FULLY INTEGRATED**
```rust
// All these are working in cli/src/batch.rs
client.submit_transaction(amount: i32) -> Result<SubmitTransactionResponse, ApiClientError>
client.get_pending_transactions() -> Result<PendingTransactionsResponse, ApiClientError>
client.create_batch(batch_size: Option<i32>) -> Result<CreateBatchResponse, ApiClientError>
client.get_batches(limit: Option<i32>) -> Result<BatchListResponse, ApiClientError>
client.get_batch(batch_id: i32) -> Result<Option<BatchInfo>, ApiClientError>
client.update_batch_proof(batch_id: i32, sindri_proof_id: String) -> Result<(), ApiClientError>
client.get_current_state() -> Result<CurrentStateResponse, ApiClientError>
client.get_contract_data(batch_id: i32) -> Result<Option<ContractSubmissionData>, ApiClientError>
client.health_check() -> Result<HealthResponse, ApiClientError>
client.get_api_info() -> Result<ApiInfoResponse, ApiClientError>
```

### User Management Operations ❌ **NEEDS INTEGRATION**
```rust
// For Agent 1 (User Management)
client.create_user(
    user_type: &str, 
    email: &str, 
    password: &str, 
    jurisdiction: Option<&str>
) -> Result<serde_json::Value, ApiClientError>

client.get_user(user_id: &str) -> Result<serde_json::Value, ApiClientError>

client.list_users(
    user_type: Option<&str>,
    kyc_status: Option<&str>, 
    limit: Option<i32>,
    offset: Option<i32>
) -> Result<serde_json::Value, ApiClientError>

client.update_user(
    user_id: &str,
    email: Option<&str>,
    user_type: Option<&str>, 
    jurisdiction: Option<&str>
) -> Result<serde_json::Value, ApiClientError>

client.delete_user(user_id: &str) -> Result<serde_json::Value, ApiClientError>

client.submit_kyc(
    user_id: &str,
    document_type: &str,
    document_hash: &str,
    jurisdiction: Option<&str>,
    kyc_investor_type: Option<&str>
) -> Result<serde_json::Value, ApiClientError>

client.update_kyc_status(
    user_id: &str,
    status: &str,
    notes: Option<&str>
) -> Result<serde_json::Value, ApiClientError>

client.generate_invite_code() -> Result<serde_json::Value, ApiClientError>

client.validate_invite_code(code: &str) -> Result<serde_json::Value, ApiClientError>
```

### Balance Operations ❌ **NEEDS INTEGRATION**
```rust
// For Agent 2 (Balance Management)
client.get_user_balance_by_address(address: &str) -> Result<UserBalanceResponse, ApiClientError>

client.get_user_balance_by_id(user_id: &str) -> Result<UserBalanceResponse, ApiClientError>

client.get_user_transactions(user_id: &str) -> Result<UserTransactionsResponse, ApiClientError>

client.get_transaction_status(tx_hash: &str) -> Result<TransactionStatusResponse, ApiClientError>

client.update_user_wallet(
    user_id: &str, 
    wallet_address: &str
) -> Result<serde_json::Value, ApiClientError>
```

### Investor Operations ❌ **NEEDS INTEGRATION**
```rust
// For Agent 4 (Investor Operations)
client.register_investor(
    email: &str,
    first_name: Option<&str>,
    last_name: Option<&str>,
    jurisdiction: Option<&str>,
    accredited_investor: Option<bool>,
    investment_capacity: Option<f64>,
    regulatory_jurisdiction: Option<&str>,
    invite_code: Option<&str>
) -> Result<serde_json::Value, ApiClientError>

client.get_investor_profile(user_id: &str) -> Result<serde_json::Value, ApiClientError>

client.update_investor_profile(
    user_id: &str,
    accredited_investor: Option<bool>,
    investment_capacity: Option<f64>,
    regulatory_jurisdiction: Option<&str>
) -> Result<serde_json::Value, ApiClientError>
```

### Pool Operations ❌ **NEEDS INTEGRATION**
```rust
// For Agent 3 (Pool Management)
client.create_pool(
    name: &str,
    max_ltv_ratio: f64,
    max_interest_rate: f64,
    repayment_period_days: i32
) -> Result<PoolResponse, ApiClientError>

client.list_pools(status: Option<&str>) -> Result<PoolListResponse, ApiClientError>

client.get_pool(pool_id: &str) -> Result<Option<PoolResponse>, ApiClientError>

client.update_pool_status(
    pool_id: &str, 
    status: &str
) -> Result<PoolResponse, ApiClientError>

client.find_matching_pools(
    required_liquidity: f64,
    max_ltv: f64,
    max_rate: f64,
    min_repayment_days: i32
) -> Result<PoolListResponse, ApiClientError>
```

## 🔗 Custom HTTP Call Pattern

For endpoints NOT available in BatchApiClient, use this pattern:

```rust
use reqwest::Client;
use serde_json::json;
use std::env;

async fn custom_api_call() -> Result<()> {
    let http_client = Client::new();
    let api_url = env::var("ARDA_CREDIT_API_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());
    
    let url = format!("{}/api/v1/custom-endpoint", api_url);
    
    let request_body = json!({
        "field1": value1,
        "field2": value2
    });

    match http_client.post(&url).json(&request_body).send().await {
        Ok(response) => {
            match response.status().as_u16() {
                200..=299 => {
                    let result: serde_json::Value = response.json().await?;
                    println!("✅ Operation successful!");
                    // Handle success response
                }
                404 => {
                    println!("❌ Resource not found");
                }
                400 => {
                    println!("❌ Invalid request parameters");
                }
                401 | 403 => {
                    println!("❌ Authentication required");
                }
                _ => {
                    let error_text = response.text().await?;
                    eprintln!("❌ API error: {}", error_text);
                    return Err(eyre::eyre!("API error: {}", error_text));
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Network error: {}", e);
            return Err(e.into());
        }
    }
    
    Ok(())
}
```

## 📊 Response Type Reference

### Common Response Structures

```rust
// User responses (serde_json::Value)
{
    "user_id": "string",
    "email": "string", 
    "user_type": "string",
    "kyc_status": "string",
    "created_at": "string",
    "updated_at": "string"
}

// Balance responses
UserBalanceResponse {
    wallet_address: String,
    user_id: Option<String>,
    available_balance: String,
    currency: String
}

// Pool responses  
PoolResponse {
    pool_id: String,
    name: String,
    investor_id: String,
    max_ltv_ratio: f64,
    max_interest_rate: f64,
    repayment_period_days: i32,
    total_deposits: f64,
    available_liquidity: f64,
    utilized_liquidity: f64,
    status: String,
    created_at: String,
    updated_at: String
}
```

## ⚠️ Known Issues to Address

1. **Missing API Endpoints**: Many operations require custom HTTP calls
2. **Authentication**: Some endpoints require authentication (not yet implemented)
3. **Error Types**: Need to handle `ApiClientError` vs custom HTTP errors
4. **Response Parsing**: serde_json::Value responses need proper field extraction

## 🎯 Success Validation

After all packages complete:

1. **Run CLI compilation**: `cargo build --bin cli`
2. **Check warnings**: Should be zero unused client warnings  
3. **Test basic operations**: User creation, balance checks, pool listing
4. **Verify error handling**: Test with API server down
5. **Check verbose output**: Ensure real data is displayed

---

**Use this reference alongside `API_INTEGRATION.md` for complete implementation guidance.**
